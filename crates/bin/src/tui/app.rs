use std::{
    error,
    future::Future,
    rc::Rc,
    sync::{Arc, Mutex},
    thread,
    thread::sleep,
};

use brontes_types::mev::{
    bundle::Bundle,
    events::{Action, TuiEvents},
    MevBlock,
};
use crossterm::event::{Event as CrosstermEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use eyre::{Context, Error, Result}; //
use itertools::Itertools;
use ratatui::{
    prelude::{Rect, *},
    widgets::*,
};
use reth_tasks::{TaskExecutor, TaskManager};
use tokio::{
    sync::{
        broadcast::Sender,
        mpsc::{self, unbounded_channel, UnboundedReceiver, UnboundedSender},
    },
    time::Duration,
};
use tracing::{info, trace};

use crate::{
    runner::CliContext,
    tui::{term::Term, tui::Event},
};

pub type AppResult<T> = std::result::Result<T, Box<dyn error::Error>>;

use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
};

use super::components::analytics::hot_tokens::HotTokens;
use crate::tui::{
    components::{
        analytics::{
            analytics::Analytics, searcher_stats::SearcherStats, top_contracts::TopContracts,
            vertically_integrated::VerticallyIntegrated,
        },
        dashboard::Dashboard,
        dbsize::DbSize,
        livestream::Livestream,
        metrics::Metrics,
        navigation::Navigation,
        settings::Settings,
        tick::Tick,
        Component,
    },
    config::Config,
    mode::Mode,
    tui,
};

#[derive(Debug)]
pub struct App {
    pub config:               Config,
    pub tick_rate:            f64,
    pub frame_rate:           f64,
    pub components:           Vec<Vec<Box<dyn Component + Send>>>,
    term:                     Term,
    pub should_quit:          bool,
    pub should_suspend:       bool,
    pub context:              Arc<Mutex<AppContext>>,
    mev_blocks:               Arc<Mutex<Vec<MevBlock>>>,
    mev_bundles:              Arc<Mutex<Vec<Bundle>>>,
    pub mode:                 Mode,
    pub last_tick_key_events: Vec<KeyEvent>,
    pub progress_counter:     Option<u16>,
}

#[derive(Debug, Clone, Default)]
pub struct AppContext {
    pub tab_index:        usize,
    pub row_index:        usize,
    pub state:            TableState,
    pub dashboard_state:  TableState,
    pub livestream_state: TableState,
}

impl App {
    pub fn new() -> Result<Self> {
        let analytics = Analytics::new();

        let dashboard = Dashboard::default();
        let livestream = Livestream::default();
        let dbsize = DbSize::default();
        let tick = Tick::default();
        let metrics = Metrics::default();
        let settings = Settings::new();
        //let tokens = Tokens::default();
        let top_contracts = TopContracts::default();
        let searcher_stats = SearcherStats::default();
        let vertically_integrated = VerticallyIntegrated::default();
        let hot_tokens = HotTokens::default();

        let context = Arc::new(Mutex::new(AppContext::default()));
        let navigation = Navigation::new(context.clone());
        let navigation_box = Box::new(navigation);
        let config = Config::new()?;
        let mode = Mode::Dashboard;
        let tick_rate = 1.0;
        let frame_rate = 60.0;

        Ok(Self {
            tick_rate,
            frame_rate,
            components: vec![
                vec![navigation_box.clone(), Box::new(dashboard)],
                vec![navigation_box.clone(), Box::new(livestream)],
                vec![
                    navigation_box.clone(),
                    Box::new(analytics),
                    Box::new(top_contracts),
                    Box::new(searcher_stats),
                    Box::new(vertically_integrated),
                    Box::new(hot_tokens),
                ],
                vec![navigation_box.clone(), Box::new(metrics), Box::new(tick), Box::new(dbsize)],
                vec![navigation_box, Box::new(settings)],
            ],
            config,
            mode,
            last_tick_key_events: Vec::new(),
            term: Term::start()?,
            should_quit: false,
            should_suspend: false,
            context,
            mev_blocks: Arc::new(Mutex::new(Vec::new())),
            mev_bundles: Arc::new(Mutex::new(Vec::new())),
            progress_counter: None,
        })
    }

    pub async fn run(rx: UnboundedReceiver<Action>, tx: UnboundedSender<Action>) {
        let mut app = Self::new().unwrap();

        if let Err(e) = app.run_inner(rx, tx).await {
            error!("TUI Error: {:?}", e);
        }
    }

    pub async fn run_inner(
        &mut self,
        mut action_rx: UnboundedReceiver<Action>,
        action_tx: UnboundedSender<Action>,
    ) -> Result<(), Error> {
        let mut tui = tui::Tui::new()?
            .tick_rate(self.tick_rate)
            .frame_rate(self.frame_rate);

        // TODO: Add mouse support & handling
        // tui.mouse(true);

        tui.enter()?;

        // register action handlers for components
        for inner_tabs in self.components.iter_mut() {
            for component in inner_tabs.iter_mut() {
                component.register_action_handler(action_tx.clone())?;
            }
        }

        // register config handlers for components
        for inner_tabs in self.components.iter_mut() {
            for component in inner_tabs.iter_mut() {
                component.register_config_handler(self.config.clone())?;
            }
        }

        //init components
        for inner_tabs in self.components.iter_mut() {
            for component in inner_tabs.iter_mut() {
                component.init(tui.size()?)?;
            }
        }

        loop {
            if let Some(e) = tui.next().await {
                match e {
                    tui::Event::Quit => action_tx.send(Action::Quit)?,
                    tui::Event::Tick => action_tx.send(Action::Tick)?,
                    tui::Event::Render => action_tx.send(Action::Render)?,
                    tui::Event::Resize(x, y) => action_tx.send(Action::Resize(x, y))?,
                    tui::Event::Key(key) => {
                        if let Some(keymap) = self.config.keybindings.get(&self.mode) {
                            if let Some(action) = keymap.get(&vec![key]) {
                                log::info!("Got action: {action:?}");
                                action_tx.send(action.clone())?;
                            } else {
                                // If the key was not handled as a single key action,
                                // then consider it for multi-key combinations.
                                self.last_tick_key_events.push(key);

                                // Check for multi-key combinations
                                if let Some(action) = keymap.get(&self.last_tick_key_events) {
                                    log::info!("Got action: {action:?}");
                                    action_tx.send(action.clone())?;
                                }
                            }
                        };
                    }
                    _ => {}
                }

                // handling events one time for each component
                let mut component_cache = HashSet::new();

                for inner_tabs in self.components.iter_mut() {
                    for component in inner_tabs.iter_mut() {
                        let component_id = component.name();
                        if !component_cache.contains(&component_id) {
                            if let Some(action) = component.handle_events(Some(e.clone()))? {
                                action_tx.send(action)?;
                                component_cache.insert(component_id); // Mark this component as processed
                            }
                        }
                    }
                }
            }

            while let Ok(action) = action_rx.try_recv() {
                if action != Action::Tick && action != Action::Render {
                    log::debug!("{action:?}");
                }
                match action {
                    Action::Tick => {
                        self.last_tick_key_events.drain(..);
                    }
                    Action::Quit => self.should_quit = true,
                    Action::Suspend => self.should_suspend = true,
                    Action::Resume => self.should_suspend = false,
                    Action::Resize(w, h) => {
                        tui.resize(Rect::new(0, 0, w, h))?;

                        tui.draw(|f| {
                            let tab_index = self.context.lock().unwrap().tab_index;

                            for component in self.components[tab_index].iter_mut() {
                                let r = component.draw(f, f.size());
                                if let Err(e) = r {
                                    action_tx
                                        .send(Action::Error(format!("Failed to draw: {:?}", e)))
                                        .unwrap();
                                }
                            }
                        })?;
                    }
                    Action::Render => {
                        tui.draw(|f| {
                            let tab_index = self.context.lock().unwrap().tab_index;

                            for component in self.components[tab_index].iter_mut() {
                                let r = component.draw(f, f.size());
                                if let Err(e) = r {
                                    action_tx
                                        .send(Action::Error(format!("Failed to draw: {:?}", e)))
                                        .unwrap();
                                }
                            }
                        })?;
                    }
                    _ => {}
                }

                // Send actions to each component
                for inner_vec in self.components.iter_mut() {
                    for component in inner_vec.iter_mut() {
                        if let Some(action) = component.update(action.clone())? {
                            action_tx.send(action)?;
                        };
                    }
                }
            }
            if self.should_suspend {
                tui.suspend()?;
                action_tx.send(Action::Resume)?;
                tui = tui::Tui::new()?
                    .tick_rate(self.tick_rate)
                    .frame_rate(self.frame_rate);
                // tui.mouse(true);
                tui.enter()?;
            } else if self.should_quit {
                tui.stop()?;
                break;
            }
        }
        tui.exit()?;
        Ok(())
    }

    pub fn next(&self) {
        let _i = match self.context.lock().unwrap().state.selected() {
            Some(i) => {
                if i >= self.mev_bundles.lock().unwrap().len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
    }

    pub fn previous(&self) {
        let _i = match self.context.lock().unwrap().state.selected() {
            Some(i) => {
                if i == 0 {
                    self.mev_bundles.lock().unwrap().len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
    }
}

/// simple helper method to split an area into multiple sub-areas
pub fn layout(area: Rect, direction: Direction, heights: Vec<u16>) -> Rc<[Rect]> {
    let constraints = heights
        .iter()
        .map(|&h| if h > 0 { Constraint::Length(h) } else { Constraint::Min(0) })
        .collect_vec();

    Layout::default()
        .direction(direction)
        .constraints(constraints)
        .split(area)
}
