mod leaderboard;
mod livestream;
mod mev_count;
mod navigation;
mod progress;

use std::{
    sync::{Arc, Mutex},
    thread, time,
};

use ansi_to_tui::IntoText;
use brontes_types::mev::{bundle::Bundle, Mev, MevBlock};
use crossterm::event::{KeyCode, KeyEvent};
use eyre::Result; //
use itertools::Itertools;
use log::*;
use polars::prelude::*;
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::UnboundedSender;
use tracing::info;
use tui_logger::*;

use super::{Component, Frame};
use crate::{
    events::*,
    get_symbols_from_transaction_accounting,
    tui::{
        config::Config,
        polars::{bundles_to_dataframe, dataframe_to_table_rows},
        theme::THEME,
        tui::Event,
    },
};

#[derive(Default, Debug)]
pub struct Dashboard {
    command_tx: Option<UnboundedSender<Action>>,
    config:     Config,
    navigation: Navigation,
    mev_count:  MevCount,
    livestream: Livestream,
    progress:   Progress,
    focus:      Focus,
}

impl Component for Dashboard {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn name(&self) -> String {
        "Dashboard".to_string()
    }

    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        match key.code {
            KeyCode::Enter => {
                self.popup_scroll_position = 0;
                self.popup_scroll_state = self
                    .popup_scroll_state
                    .position(self.popup_scroll_position as usize);

                self.show_popup = !self.show_popup;
            }
            KeyCode::Up => {
                self.next();
            }
            KeyCode::Down => {
                self.previous();
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
            Action::Tui(tui_event) => {
                match tui_event {
                    TuiEvents::MevBlockMetricReceived(mevblock) => {
                        let mut blocks: std::sync::MutexGuard<'_, Vec<MevBlock>> =
                            self.mevblocks.lock().unwrap();
                        blocks.push(mevblock.clone()); // Store received block
                    }
                    TuiEvents::MevBundleEventReceived(bundle) => {
                        let mut bundles: std::sync::MutexGuard<'_, Vec<Bundle>> =
                            self.mev_bundles.lock().unwrap();
                        bundles.extend(bundle.into_iter());
                    }
                }
            }
            _ => {}
        }
        Ok(None)
    }

    fn init(&mut self, _area: Rect) -> Result<()> {
        Ok(())
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let area = area.inner(&Margin { vertical: 1, horizontal: 4 });

        let template = Layout::default()
            .constraints([
                Constraint::Length(1),
                Constraint::Min(8),
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .split(area);

        let chunks = Layout::default()
            .constraints([Constraint::Length(9), Constraint::Min(20), Constraint::Length(8)])
            .split(template[1]);

        let sub_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[0]);

        let buf = f.buffer_mut();

        Self::draw_charts(self, sub_layout[0], buf);
        Self::draw_leaderboard(self, sub_layout[1], buf);
        Self::draw_livestream(self, chunks[1], buf);
        Self::draw_logs(self, chunks[2], buf);
        Self::render_progress(self, template[2], buf);
        Self::render_bottom_bar(self, template[3], buf);
        if self.show_popup {
            if let Some(_selected_index) = self.stream_table_state.selected() {
                let block = Block::default()
                    .title("MEV Details")
                    .borders(Borders::ALL)
                    .padding(Padding::horizontal(4));

                let area = Self::centered_rect(80, 80, area);
                //Self::show_popup(self,area);
                f.render_widget(Clear, area); //this clears out the background
                let paragraph = Paragraph::new("Hello, world!");
                f.render_widget(paragraph, area);

                let mevblocks_guard: std::sync::MutexGuard<'_, Vec<Bundle>> =
                    self.mev_bundles.lock().unwrap();

                let text = mevblocks_guard[self.stream_table_state.selected().unwrap()]
                    .to_string()
                    .into_text();

                let paragraph = Paragraph::new(text.unwrap())
                    .block(block)
                    .scroll((self.popup_scroll_position, 0));

                f.render_widget(paragraph, area);

                f.render_stateful_widget(
                    Scrollbar::default()
                        .orientation(ScrollbarOrientation::VerticalLeft)
                        .begin_symbol(Some("↑"))
                        .end_symbol(Some("↓")),
                    f.size().inner(&Margin { vertical: 10, horizontal: 10 }),
                    &mut self.popup_scroll_state,
                );
            }
            //f.render_widget(block, area);
        }

        Ok(())
    }
}

impl Dashboard {
    fn render_bottom_bar(&self, area: Rect, buf: &mut Buffer) {
        let keys = [
            ("Q/Esc", "Quit"),
            ("Tab", "Next Tab"),
            ("BackTab", "Previous Tab"),
            ("↑/w", "Up"),
            ("↓/s", "Down"),
            ("↵", "Open/Close Mev Details"),
        ];
        let spans = keys
            .iter()
            .flat_map(|(key, desc)| {
                let key = Span::styled(format!(" {} ", key), THEME.key_binding.key);
                let desc = Span::styled(format!(" {} ", desc), THEME.key_binding.description);
                [key, desc]
            })
            .collect_vec();
        Paragraph::new(Line::from(spans))
            .alignment(Alignment::Center)
            .fg(Color::Indexed(236))
            .bg(Color::Indexed(232))
            .render(area, buf);
    }
}

pub enum Focus {
    #[derive(Default)]
    Dashboard,
    Livestream,
    Progress,
    Logs,
    MevCount,
    LeaderBoard,
}
