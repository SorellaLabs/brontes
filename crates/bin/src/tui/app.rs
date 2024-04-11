use std::{
    error,
    future::Future,
    io::{self, stdout, Stdout},
    ops::{Deref, DerefMut},
    rc::Rc,
    sync::Arc,
    task::Poll,
    thread,
    thread::sleep,
    time::Duration,
};

use brontes_database::tui::events::TuiUpdate;
use brontes_types::mev::{bundle::Bundle, MevBlock};
use crossterm::{
    event::{self, Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use eyre::{Context, Error, Result};
use futures::{StreamExt, TryStreamExt};
use itertools::Itertools;
use parking_lot::Mutex;
use polars::frame::DataFrame;
use ratatui::{
    prelude::{Color, Constraint, Direction, Layout, Rect, Style, *},
    widgets::{Block, Borders, ScrollbarState, *},
};
use reth_tasks::{TaskExecutor, TaskManager};
use tokio::sync::{
    broadcast::Sender,
    mpsc::{self, unbounded_channel, UnboundedReceiver, UnboundedSender},
};
use tracing::{error, info, trace};

use crate::runner::CliContext;

pub type AppResult<T> = std::result::Result<T, Box<dyn error::Error>>;

use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
};

//use super::{components::analytics::hot_tokens::HotTokens, mode::Page};
use crate::tui::{
    components::{
        dashboard::Dashboard,
        /*analytics::{
            analytics::Analytics, searcher_stats::SearcherStats, top_contracts::TopContracts,
            vertically_integrated::VerticallyIntegrated,
        }*/
        metrics::Metrics, Component,
    },
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

    // Return state to the poll loop & handle graceful shutdown
    pub fn handle_key_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Key(key) => match key.code {
                KeyCode::BackTab => {
                    self.page_index.saturating_sub(1) % TAB_COUNT;
                    self.term
                        .draw(|f| self.components[self.page_index].on_select(f));
                }
                KeyCode::Tab => {
                    self.page_index.saturating_add(1) % TAB_COUNT;
                    self.term
                        .draw(|f| self.components[self.page_index].on_select(f));
                }
                KeyCode::Char('q') => {
                    stop_terminal()?;
                }
                _ => self.components[self.page_index].handle_key_events(key),
            },
            Event::Resize(width, height) => {
                //TODO: handle resize by redrawing on active componentJ
                self.term.resize(Rect::new(0, 0, width, height));
            }
            _ => {}
        }
        Ok(())
    }
}

impl Future for App {
    //TODO: Use app output to send signal back to main & control graceful shutdown
    // or reruns based on user input via settings
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        loop {
            if let Poll::Ready(item) = self.events.poll_next_unpin(cx) {
                match item {
                    Some(Ok(event)) => self
                        .handle_key_event(event)
                        .expect("Panicked handling key event"),
                    Some(Err(e)) => panic!("Error: {:?}", e),
                    None => return Poll::Ready(()),
                }
            }
        }
    }
}

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
