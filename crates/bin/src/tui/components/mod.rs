use std::{fmt, fmt::Debug};

use brontes_database::tui::events::TuiUpdate;
use color_eyre::eyre::Result;
use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::layout::Rect;


pub mod shared;

use crossterm::event::Event;

use crate::tui::config::Config;
pub mod ClickableList;
pub mod analytics;
pub mod constants;
pub mod dashboard;
pub mod livestream;
pub mod metrics;
pub mod settings;
pub mod tick;
pub mod tokens;

pub type Frame<'a> = ratatui::Frame<'a>;

pub trait Component: Debug {
    #[allow(unused_variables)]
    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        Ok(())
    }

    fn name(&self) -> String {
        "".to_string()
    }

    fn handle_events(&mut self, event: Option<Event>) {
        let r = match event {
            Some(Event::Key(key_event)) => self.handle_key_events(key_event),
            Some(Event::Mouse(mouse_event)) => self.handle_mouse_events(mouse_event),
            _ => (),
        };
    }

    #[allow(unused_variables)]
    fn handle_key_events(&mut self, key: KeyEvent) {}

    #[allow(unused_variables)]
    fn handle_mouse_events(&mut self, mouse: MouseEvent) {}

    fn handle_data_events(&mut self, event: TuiUpdate) {}

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) {}

    fn on_select(&mut self, f: &mut Frame<'_>) {}

    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Component")
            // Add other fields here
            .finish()
    }
}
