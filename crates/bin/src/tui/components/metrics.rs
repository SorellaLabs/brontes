use std::{collections::HashMap, time::Duration};

use brontes_types::mev::events::Action;
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use super::{Component, Frame};
use crate::tui::config::{Config, KeyBindings};

#[derive(Default, Debug)]
pub struct Metrics {
    command_tx: Option<UnboundedSender<Action>>,
    config:     Config,
}

impl Metrics {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Component for Metrics {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn name(&self) -> String {
        "metrics".to_string()
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Tick => {}
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
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
        Ok(())
    }
}
