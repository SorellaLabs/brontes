use std::{
    ops::{Deref, DerefMut},
    time::Duration,
};

use color_eyre::eyre::Result;
use crossterm::{
    cursor,
    event::{
        DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event as CrosstermEvent, EventStream, KeyEvent, KeyEventKind, MouseEvent,
    },
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::{FutureExt, Stream, StreamExt};
use ratatui::{backend::CrosstermBackend as Backend, widgets::Widget};
use serde::{Deserialize, Serialize};
use tokio::{
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

pub type IO = std::io::Stderr;
pub fn io() -> IO {
    std::io::stderr()
}
pub type Frame<'a> = ratatui::Frame<'a>;

pub struct Tui {
    pub terminal: ratatui::Terminal<Backend<IO>>,
    pub mouse:    bool,
    pub paste:    bool,
}

impl Tui {
    pub fn new() -> Result<Self> {
        let terminal = ratatui::Terminal::new(Backend::new(io()))?;
        let mouse = false;
        let paste = false;

        Ok(Self { terminal, mouse, paste })
    }

    pub fn mouse(mut self, mouse: bool) -> Self {
        self.mouse = mouse;
        self
    }

    pub fn paste(mut self, paste: bool) -> Self {
        self.paste = paste;
        self
    }
}
