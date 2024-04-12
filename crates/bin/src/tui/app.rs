use std::{
    error,
    future::Future,
    io::{self, stdout, Stdout},
    pin::Pin,
    rc::Rc,
    task::Poll,
};

use brontes_database::tui::events::TuiUpdate;
use crossterm::{
    event::{Event, EventStream, KeyCode, KeyEvent},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use eyre::{Context, Result};
use futures::StreamExt;
use itertools::Itertools;
use ratatui::prelude::{Constraint, Direction, Layout, Rect, *};
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::error;

pub type AppResult<T> = std::result::Result<T, Box<dyn error::Error>>;

//use super::{components::analytics::hot_tokens::HotTokens, mode::Page};
use crate::tui::{
    components::{dashboard::Dashboard, Component},
    config::Config,
};
const TAB_COUNT: usize = 5;

#[derive(Debug)]
pub struct App {
    pub config:               Config,
    pub components:           Vec<Box<dyn Component + Send>>,
    term:                     Terminal<CrosstermBackend<Stdout>>,
    pub page_index:           usize,
    pub last_tick_key_events: Vec<KeyEvent>,
    pub events:               EventStream,
    tui_rx:                   UnboundedReceiver<TuiUpdate>,
}

impl App {
    pub fn new(rx: UnboundedReceiver<TuiUpdate>) -> Result<Self> {
        let config = Config::new()?;

        Ok(Self {
            events: EventStream::new(),
            components: vec![Box::new(Dashboard::new())],
            config,
            page_index: 0,
            last_tick_key_events: Vec::new(),
            term: start_terminal()?,
            tui_rx: rx,
        })
    }

    pub fn handle_terminal_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Key(key) => self.handle_key_event(key),
            Event::Resize(width, height) => {
                self.term.resize(Rect::new(0, 0, width, height));
                self.term
                    .draw(|f| self.components[self.page_index].on_select(f));
                Ok(())
            }
            _ => Ok(()),
        }
    }

    pub fn handle_data_event(&mut self, event: TuiUpdate) -> Result<()> {
        match self.page_index {
            0 => self.components[self.page_index].handle_data_events(event),
            _ => Ok(()),
        }
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<()> {
        match key_event.code {
            KeyCode::BackTab => {
                self.page_index.saturating_sub(1) % TAB_COUNT;
                self.term
                    .draw(|f| self.components[self.page_index].on_select(f));
                Ok(())
            }
            KeyCode::Tab => {
                self.page_index.saturating_add(1) % TAB_COUNT;
                self.term
                    .draw(|f| self.components[self.page_index].on_select(f));
                Ok(())
            }
            KeyCode::Char('q') => stop_terminal(),
            _ => self.components[self.page_index].handle_key_events(key_event),
        }
    }
}

impl Future for App {
    //TODO: Use app output to send signal back to main & control graceful shutdown
    // or reruns based on user input via settings
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        while let Poll::Ready(option) = self.events.poll_next_unpin(cx) {
            match option {
                Some(Ok(event)) => {
                    if let Err(e) = self.handle_terminal_event(event) {
                        error!("Failed to handle terminal event: {:?}", e);
                    }
                }
                Some(Err(e)) => {
                    error!("Error in terminal events stream: {:?}", e);
                    return Poll::Ready(());
                }
                None => return Poll::Ready(()),
            }
        }

        while let Poll::Ready(option) = self.tui_rx.poll_recv(cx) {
            match option {
                Some(event) => {
                    if let Err(e) = self.handle_data_events(event) {
                        error!("Failed to handle data event: {:?}", e);
                    }
                }
                None => return Poll::Ready(()),
            }
        }

        Poll::Pending
    }
}

pub fn start_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    let options = TerminalOptions { viewport: Viewport::Fullscreen };
    let terminal = Terminal::with_options(CrosstermBackend::new(io::stdout()), options)?;
    enable_raw_mode().context("enable raw mode")?;
    stdout()
        .execute(EnterAlternateScreen)
        .context("enter alternate screen")?;
    Ok(terminal)
}

pub fn stop_terminal() -> Result<()> {
    disable_raw_mode().context("disable raw mode")?;
    stdout()
        .execute(LeaveAlternateScreen)
        .context("leave alternate screen")?;
    Ok(())
}
