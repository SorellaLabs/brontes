use std::{collections::HashMap, time::Duration};

use brontes_types::mev::events::Action;
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use crate::tui::{
    components::{Component, Frame},
    config::{Config, KeyBindings},
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
