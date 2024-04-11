mod db;

use brontes_database::tui::events::TuiUpdate;
use color_eyre::eyre::Result;
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::UnboundedSender;

use super::{Component, Frame};
use crate::tui::config::Config;

#[derive(Default, Debug)]
pub struct Metrics {
    command_tx: Option<UnboundedSender<TuiUpdate>>,
    config:     Config,
}

impl Metrics {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Component for Metrics {
    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn name(&self) -> String {
        "metrics".to_string()
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) {
        let rect = area.inner(&Margin { vertical: 1, horizontal: 4 });

        let rects = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Length(3), // first row
                Constraint::Min(0),
            ])
            .split(rect);

        let rect = rects[1];

        f.render_widget(
            Paragraph::new(
                "Prometheus is up and is collecting data. Start a grafana instance and import xyz",
            ),
            rect,
        );
    }
}
