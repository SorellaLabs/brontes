use std::{
    sync::{Arc, Mutex},
};

use brontes_types::mev::events::Action;
use color_eyre::eyre::Result;
use crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers,
};
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::UnboundedSender;

use super::{Component, Frame};
use crate::tui::app::AppContext;
use crate::tui::{
    //events::{Event, EventHandler},
    app::layout,
    config::{Config},
    theme::THEME,
    tui::Event,
};

#[derive(Clone, Debug)]
pub struct Navigation {
    command_tx: Option<UnboundedSender<Action>>,
    config:     Config,
    context:    Arc<Mutex<AppContext>>,
}

impl Navigation {
    pub fn new(context: Arc<Mutex<AppContext>>) -> Self {
        Self { command_tx: Default::default(), config: Default::default(), context }
    }

    fn render_title_bar(&self, area: Rect, buf: &mut Buffer) {
        let area = layout(area, Direction::Horizontal, vec![0, 58]);
        let tab_index = self.context.lock().unwrap().tab_index;
        Paragraph::new(Span::styled("Brontes", THEME.app_title)).render(area[0], buf);
        let titles = vec!["DASHBOARD", " LIVESTREAM ", " ANALYTICS ", " METRICS ", " SETTINGS "];
        Tabs::new(titles)
            .style(THEME.tabs)
            .highlight_style(THEME.tabs_selected)
            .select(tab_index)
            .divider("")
            .render(area[1], buf);
    }
}

impl Component for Navigation {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn name(&self) -> String {
        "navigation".to_string()
    }

    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        let TAB_COUNT = 5;

        match key.code {
            KeyCode::Tab | KeyCode::BackTab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                let mut appcontext_lock = self.context.lock().unwrap();
                appcontext_lock.tab_index = appcontext_lock.tab_index.saturating_sub(1) % TAB_COUNT;
            }
            KeyCode::Tab | KeyCode::BackTab => {
                let mut appcontext_lock = self.context.lock().unwrap();
                appcontext_lock.tab_index = appcontext_lock.tab_index.saturating_add(1) % TAB_COUNT;
            }

            _ => (),
        };

        Ok(Some(Action::Tick))
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
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let area = area.inner(&Margin { vertical: 1, horizontal: 4 });

        let template = Layout::default()
            .constraints([Constraint::Length(1), Constraint::Min(8), Constraint::Length(1)])
            .split(area);
        let buf = f.buffer_mut();

        Self::render_title_bar(self, template[0], buf);

        Ok(())
    }
}
