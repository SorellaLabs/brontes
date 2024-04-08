use std::{
    borrow::{Borrow, BorrowMut},
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use brontes_metrics::PoirotMetricsListener;
use brontes_types::mev::{
    bundle::Bundle,
    events::{Action, TuiEvents},
    MevBlock,
};
use crossterm::event::{
    Event as CrosstermEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent,
};
use eyre::{Context, Error, Result}; //
use itertools::Itertools;
use lazy_static::lazy_static;
use log::{LevelFilter, *};
use ratatui::{
    prelude::*,
    text::Line,
    widgets::{canvas::*, *},
};
use serde::{Deserialize, Serialize};
use tokio::sync::{
    broadcast::Sender,
    mpsc::{unbounded_channel, UnboundedSender},
};
use tracing::info;
use tracing_log::{self, LogTracer};
use tracing_subscriber::{
    fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer, Registry,
};
use tui_logger::{self, *};

use super::{Component, Frame};
use crate::tui::{
    colors::RgbSwatch,
    config::{Config, KeyBindings},
    //events::{Event, EventHandler},
    root::layout,
    theme::THEME,
    tui::Event,
};

use crate::tui::components::{
    dashboard::Dashboard, livestream::Livestream, analytics::Analytics, metrics::Metrics, settings::Settings,
    tokens::Tokens,
};



#[derive(Default, Debug)]

pub struct Root {
    command_tx:  Option<UnboundedSender<Action>>,
    config:      Config,
    tab_index:      usize,
    mev_blocks:  Arc<Mutex<Vec<MevBlock>>>,
    mev_bundles: Arc<Mutex<Vec<Bundle>>>,
}

/*
pub struct Root<'a> {
    context: &'a AppContext,
    mev_blocks: Arc<Mutex<Vec<MevBlock>>>,
    mev_bundles: Arc<Mutex<Vec<Bundle>>>,
    //events: Sender<Event>, // Removed the mutable reference since `Sender` can be cloned for sending messages
}

impl<'a> Root<'a> {
    // Simplified to take owned `Arc<Mutex<...>>` and `Sender<Event>` directly
    pub fn new(context: &'a AppContext, mev_blocks: Arc<Mutex<Vec<MevBlock>>>, mev_bundles: Arc<Mutex<Vec<Bundle>>>) -> Self {
        Root { context, mev_blocks, mev_bundles }
    }
}

impl<'a> Widget for Root<'a> {
    fn render(mut self, area: Rect, buf: &mut Buffer) {
        //println!("mev_blocks: {:?}", self.mev_blocks.lock().unwrap());
        Block::new().style(THEME.root).render(area, buf);
        let area = layout(area, Direction::Vertical, vec![1, 0, 1]);
        self.render_title_bar(area[0], buf);
        self.render_selected_tab(area[1], buf);
        self.render_bottom_bar(area[2], buf);
    }
}
*/

impl Root {
    pub fn new(  mev_blocks: Arc<Mutex<Vec<MevBlock>>>, mev_bundles: Arc<Mutex<Vec<Bundle>>>) -> Self {
        Root {   mev_blocks, mev_bundles,             ..Default::default()}
    }

    fn render_title_bar(&self, area: Rect, buf: &mut Buffer) {
        let area = layout(area, Direction::Horizontal, vec![0, 58]);

        Paragraph::new(Span::styled("Brontes", THEME.app_title)).render(area[0], buf);
        let titles = vec!["DASHBOARD", " LIVESTREAM ", " ANALYTICS ", " METRICS ", " SETTINGS "];
        Tabs::new(titles)
            .style(THEME.tabs)
            .highlight_style(THEME.tabs_selected)
            .select(self.tab_index)
            .divider("")
            .render(area[1], buf);
    }
/*
    fn render_selected_tab(&mut self, area: Rect, buf: &mut Buffer) {
        let tab_index: usize = self.tab_index;

        //let scroll_state = self.context.scroll_state;

        let mevblocks = self.mev_blocks.clone();
        let mevbundles = self.mev_bundles.clone();

        match self.tab_index {
            0 => Dashboard::new(..Default::default()).draw(area, buf),
            1 => Livestream::new().draw(area, buf),
            2 => Analytics::new().draw(area, buf),
            3 => Metrics::new().draw(area, buf),
            4 => Settings::new().draw(area, buf),
            _ => unreachable!(),
        };
    }
*/
    fn render_bottom_bar(&self, area: Rect, buf: &mut Buffer) {
        let keys = [("Q/Esc", "Quit"), ("Tab", "Next Tab"), ("↑/k", "Up"), ("↓/j", "Down")];
        let spans = keys
            .iter()
            .flat_map(|(key, desc)| {
                let key = Span::styled(format!(" {} ", key), THEME.key_binding.key);
                let desc = Span::styled(format!(" {} ", desc), THEME.key_binding.description);
                [key, desc]
            })
            .collect_vec();
        Paragraph::new(Line::from(spans))
            .alignment(Alignment::Center)
            .fg(Color::Indexed(236))
            .bg(Color::Indexed(232))
            .render(area, buf);
    }
}

impl Component for Root {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {

        const tab_count: usize = 5;


        match key.code {
            
            KeyCode::Tab | KeyCode::BackTab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                let tab_index = self.tab_index + TAB_COUNT; // to wrap around properly
                self.tab_index = tab_index.saturating_sub(1) % tab_count;
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.tab_index = self.tab_index.saturating_add(1) % tab_count;
            }



            _ => (),
        };

        Ok(Some(Action::Tick))
    }


    fn init(&mut self, area: Rect) -> Result<()> {
      //  Root::new( self.mevblocks.clone(), self.mev_bundles.clone());
        Ok(())
      }


    fn handle_events(&mut self, event: Option<Event>) -> Result<Option<Action>> {
        let r = match event {
            Some(Event::Key(key_event)) => self.handle_key_events(key_event)?,
            Some(Event::Mouse(mouse_event)) => self.handle_mouse_events(mouse_event)?,
            _ => None,
        };
        Ok(r)
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Tick => {}
            Action::Tui(tui_event) => {
                info!("Tui event: received");
          
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {


        let tab_index: usize = self.tab_index;


/*

  let tab_index: usize = self.tab_index;

        //let scroll_state = self.context.scroll_state;

        let mevblocks = self.mev_blocks.clone();
        let mevbundles = self.mev_bundles.clone();

        match self.tab_index {
            0 => Dashboard::new(..Default::default()).draw(area, buf),
            1 => Livestream::new().draw(area, buf),
            2 => Analytics::new().draw(area, buf),
            3 => Metrics::new().draw(area, buf),
            4 => Settings::new().draw(area, buf),
            _ => unreachable!(),
        };

*/


        let mut buf  = f.buffer_mut();
        Block::new().style(THEME.root).render(area, buf);
        let area = layout(area, Direction::Vertical, vec![1, 0, 1]);
        self.render_title_bar(area[0], buf);
        //self.render_selected_tab(area[1], buf);

        match self.tab_index {
            0 => Dashboard::new(Default::default(),Default::default(),Default::default()).draw(f, area[1]),
            1 => Livestream::new().draw(f, area[1]),
            2 => Analytics::new().draw(f, area[1]),
            3 => Metrics::new().draw(f, area[1]),
            4 => Settings::new().draw(f, area[1]),
            _ => unreachable!(),
        };


        self.render_bottom_bar(area[2], buf);

        Ok(())
    }
}



impl Widget for Root {
    fn render(mut self, area: Rect, buf: &mut Buffer) {
        let area = area.inner(&Margin { vertical: 1, horizontal: 4 });

        let template = Layout::default()
            .constraints([Constraint::Length(1), Constraint::Min(8), Constraint::Length(1)])
            .split(area);

        let chunks = Layout::default()
            .constraints([Constraint::Length(9), Constraint::Min(8), Constraint::Length(20)])
            .split(template[1]);

        let sub_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[0]);

    }
}
