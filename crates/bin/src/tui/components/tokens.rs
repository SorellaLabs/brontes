use brontes_database::tui::events::TuiUpdate;
use color_eyre::eyre::Result;
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::UnboundedSender;

use super::{Component, Frame};
use crate::tui::config::Config;

#[derive(Default, Debug)]
pub struct Tokens {
    config: Config,
}

impl Tokens {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Component for Tokens {
    fn name(&self) -> String {
        "tokens".to_string()
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn handle_data_events(&mut self, _event: TuiUpdate) {}

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) {
        f.render_widget(Paragraph::new("Tokens component"), area);
    }
}
