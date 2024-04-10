use brontes_database::tui::events::Action;
use color_eyre::eyre::Result;
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::UnboundedSender;

use crate::tui::{
    components::{Component, Frame},
    config::Config,
};

#[derive(Default, Debug)]
pub struct Analytics {
    command_tx: Option<UnboundedSender<Action>>,
    config:     Config,
}

impl Analytics {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Component for Analytics {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn name(&self) -> String {
        "Analytics".to_string()
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Tick => {}
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        f.render_widget(Paragraph::new("Analytics component"), area);
        Ok(())
    }
}
