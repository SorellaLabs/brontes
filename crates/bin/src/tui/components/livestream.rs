use std::{
    future::Future,
    sync::{Arc, Mutex},
};

use ansi_to_tui::IntoText;
use brontes_types::mev::{
    bundle::Bundle,
    events::{Action, TuiEvents},
    Mev, MevBlock,
};
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use polars::frame::DataFrame;
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tracing::info;

use super::Component;
use crate::{
    get_symbols_from_transaction_accounting,
    tui::{
        async_interfaces::{ComponentUpdater, HeadAsyncComponent},
        tui::{Event, Frame},
    },
};

#[derive(Default, Debug)]
pub struct Livestream {
    #[allow(dead_code)]
    command_tx:  Option<UnboundedSender<Action>>,
    mevblocks:   Arc<Mutex<Vec<MevBlock>>>,
    mev_bundles: DataFrame,
    updater:     ComponentUpdater<Action, DataFrame>,

    terminal: Arc<parking_lot::Mutex<Tui>>,

    stream_table_state: TableState,
    show_popup:         bool,

    pub popup_scroll_position: u16,
    pub popup_scroll_state:    ScrollbarState,

    /// has a keystroke occurred that has caused a re-render
    pub kestroke_rerender: bool,
}

impl Livestream {
    pub fn new(
        mevblocks: Arc<Mutex<Vec<MevBlock>>>,
        rx: UnboundedReceiver<Action>,
        terminal: Arc<parking_lot::Mutex<Tui>>,
    ) -> Self {
        let mut stream = UnboundedReceiverStream::new(rx);
        let stream = Box::pin(stream.filter(|action| {
            // filter here
            true
        }));

        let updater = ComponentUpdater::new(stream, Box::new(Self::process_new_data));

        Self {
            terminal,
            mevblocks,
            updater,
            mev_bundles: DataFrame::default(),
            show_popup: false,
            stream_table_state: TableState::default().with_selected(Some(0)),
            ..Default::default()
        }
    }

    pub async fn process_new_data(action: Action) -> DataFrame {
        todo!(" whatever needs to be done to clean / manipulate data");
    }

    pub fn next(&mut self) {
        if self.show_popup {
            self.popup_scroll_position = self.popup_scroll_position.saturating_sub(1);
            self.popup_scroll_state = self
                .popup_scroll_state
                .position(self.popup_scroll_position as usize);
        } else {
            let i = match self.stream_table_state.selected() {
                Some(i) => {
                    let mevblocks_guard: std::sync::MutexGuard<'_, Vec<Bundle>> =
                        self.mev_bundles.lock().unwrap();

                    if mevblocks_guard.len() > 0 {
                        if i == 0 {
                            mevblocks_guard.len() - 1
                        } else {
                            i - 1
                        }
                    } else {
                        0
                    }
                }
                None => 0,
            };
            self.stream_table_state.select(Some(i));
        }
    }

    pub fn previous(&mut self) {
        if self.show_popup {
            self.popup_scroll_position = self.popup_scroll_position.saturating_add(1);
            self.popup_scroll_state = self
                .popup_scroll_state
                .position(self.popup_scroll_position as usize);
        } else {
            let i = match self.stream_table_state.selected() {
                Some(i) => {
                    let mevblocks_guard: std::sync::MutexGuard<'_, Vec<Bundle>> =
                        self.mev_bundles.lock().unwrap();

                    if mevblocks_guard.len() > 0 {
                        if i >= mevblocks_guard.len() - 1 {
                            0
                        } else {
                            i + 1
                        }
                    } else {
                        0
                    }
                }
                None => 0,
            };

            self.stream_table_state.select(Some(i));
        }
    }

    /// helper function to create a centered rect using up certain percentage of
    /// the available rect `r`
    fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(r);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }

    fn draw_livestream(&mut self, area: Rect, buf: &mut Buffer) {
        let selected_style = Style::default().add_modifier(Modifier::REVERSED);
        let normal_style = Style::default().bg(Color::Blue);

        let header_cells = [
            "Block#",
            "Tx Index",
            "MEV Type",
            "Tokens",
            "Protocols",
            "From",
            "Mev Contract",
            "Profit",
            "Cost",
        ]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::White)));
        let header = Row::new(header_cells)
            .style(normal_style)
            .height(1)
            .bottom_margin(1);

        let rows = dataframe_to_table_rows(&self.mev_bundles);

        let t = Table::new(
            rows,
            [
                Constraint::Max(10),
                Constraint::Min(5),
                Constraint::Min(20),
                Constraint::Min(20),
                Constraint::Min(20),
                Constraint::Min(32),
                Constraint::Min(32),
                Constraint::Max(10),
                Constraint::Max(10),
            ],
        )
        .header(header)
        .block(Block::default().borders(Borders::ALL).title("Live Stream"))
        .highlight_style(selected_style)
        .highlight_symbol(">> ");

        ratatui::widgets::StatefulWidget::render(t, area, buf, &mut self.stream_table_state);
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        // f.render_widget(self,area,);
        // self.render(area,f.buffer_mut());

        let area = area.inner(&Margin { vertical: 1, horizontal: 4 });

        let template = Layout::default()
            .constraints([Constraint::Length(1), Constraint::Min(8), Constraint::Length(1)])
            .split(area);

        let buf = f.buffer_mut();

        self.draw_livestream(template[1], buf);

        if self.show_popup {
            let block = Block::default()
                .title("MEV Details")
                .borders(Borders::ALL)
                .padding(Padding::horizontal(4));

            let area = Self::centered_rect(80, 80, area);
            f.render_widget(Clear, area); //this clears out the background

            let paragraph = Paragraph::new("Hello, world!");
            f.render_widget(paragraph, area);

            let text = self
                .mev_bundles
                .get_row(self.stream_table_state.selected().unwrap())
                .unwrap()
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

        Ok(())
    }
}

impl HeadAsyncComponent for Livestream {
    fn handle_key_events(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                info!("Esc pressed");
            }
            KeyCode::Enter => {
                info!("Enter pressed");

                self.popup_scroll_position = 0;
                self.popup_scroll_state = self
                    .popup_scroll_state
                    .position(self.popup_scroll_position as usize);

                self.show_popup = !self.show_popup;
                self.kestroke_rerender |= true;
            }
            KeyCode::Up => {
                self.next();
                self.kestroke_rerender |= true;
            }
            KeyCode::Down => {
                //info!("Down pressed");
                self.previous();
                self.kestroke_rerender |= true;
            }
            _ => (),
        };
    }

    fn poll_with_ctx(&mut self, should_render: bool, cx: &mut Context<'_>) -> Poll<()> {
        if let Poll::Ready(Some(updated_data)) = self.updater.poll_next_unpin(cx) {
            self.mev_bundles.extend(&updated_data).unwrap();
            if should_render {
                self.terminal.lock().draw(|f| {
                    self.draw(f, f.size());
                })
            }
            // reschedule since we resolved
            cx.waker().wake_by_ref();
            return Poll::Pending
        }

        if self.should_render && self.keystroke_rerender {
            self.keystroke_rerender = false;

            self.terminal.lock().draw(|f| {
                self.draw(f, f.size());
            })
        }

        Poll::Pending
    }
}
