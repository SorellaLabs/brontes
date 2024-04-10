use std::sync::{Arc, Mutex};

use ansi_to_tui::IntoText;
use brontes_types::mev::{
    bundle::Bundle,
    events::{Action, TuiEvents},
    Mev, MevBlock,
};
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::UnboundedSender;
use tracing::info;

use super::Component;
use crate::{
    get_symbols_from_transaction_accounting,
    tui::tui::{Event, Frame},
};

#[derive(Default, Debug)]
pub struct Livestream {
    #[allow(dead_code)]
    command_tx:                Option<UnboundedSender<Action>>,
    mevblocks:                 Arc<Mutex<Vec<MevBlock>>>,
    mev_bundles:               Arc<Mutex<Vec<Bundle>>>, // Shared state for MevBlocks
    stream_table_state:        TableState,
    show_popup:                bool,
    pub popup_scroll_position: u16,
    pub popup_scroll_state:    ScrollbarState,
}

impl Livestream {
    pub fn new(mevblocks: Arc<Mutex<Vec<MevBlock>>>, mev_bundles: Arc<Mutex<Vec<Bundle>>>) -> Self {
        Self {
            mevblocks,
            mev_bundles,
            show_popup: false,
            stream_table_state: TableState::default().with_selected(Some(0)),
            ..Default::default()
        }
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

    fn draw_livestream(widget: &mut Livestream, area: Rect, buf: &mut Buffer) {
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

        let mevblocks_guard: std::sync::MutexGuard<'_, Vec<Bundle>> =
            widget.mev_bundles.lock().unwrap();

        let rows = mevblocks_guard.iter().map(|item| {
            let protocols = item.data.protocols();
            let mut protocol_names = protocols.iter().map(|p| p.to_string()).collect::<Vec<_>>();
            protocol_names.sort();
            let protocol_list = protocol_names.join(", ");

            let height = 1;
            let cells = vec![
                item.header.block_number.to_string(),
                item.header.tx_index.to_string(),
                item.header.mev_type.to_string(),
                get_symbols_from_transaction_accounting!(&item.header.balance_deltas),
                protocol_list,
                item.header.eoa.to_string(),
                item.header
                    .mev_contract
                    .as_ref()
                    .map(|address| address.to_string())
                    .unwrap_or("Not an Mev Contract".to_string()),
                item.header.profit_usd.to_string(),
                item.header.bribe_usd.to_string(),
            ]
            .iter()
            .map(|s| Cell::from(s.to_string())) // Convert each String to a Cell
            .collect::<Vec<Cell>>(); // Collect into a Vec<Cell>

            Row::new(cells).height(height as u16).bottom_margin(0)
        });

        let t = Table::new(
            rows,
            [
                Constraint::Max(10),
                Constraint::Max(5),
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

        ratatui::widgets::StatefulWidget::render(t, area, buf, &mut widget.stream_table_state);
    }
}

impl Component for Livestream {
    fn name(&self) -> String {
        "Livestream".to_string()
    }

    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
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
            }
            KeyCode::Up => {
                //info!("Up pressed");

                self.next();
            }
            KeyCode::Down => {
                //info!("Down pressed");

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
                // info!("Tui event: received");
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
                    } //_ => (),
                }
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        // f.render_widget(self,area,);
        // self.render(area,f.buffer_mut());

        let area = area.inner(&Margin { vertical: 1, horizontal: 4 });

        let template = Layout::default()
            .constraints([Constraint::Length(1), Constraint::Min(8), Constraint::Length(1)])
            .split(area);

        let buf = f.buffer_mut();

        Self::draw_livestream(self, template[1], buf);

        if self.show_popup {
            let block = Block::default()
                .title("MEV Details")
                .borders(Borders::ALL)
                .padding(Padding::horizontal(4));

            let area = Self::centered_rect(80, 80, area);
            f.render_widget(Clear, area); //this clears out the background

            let paragraph = Paragraph::new("Hello, world!");
            f.render_widget(paragraph, area);

            /*
                        // why is this here?
                        match self.stream_table_state.selected() {
                            Some(i) => self.stream_table_state.selected(),
                            None => None,
                        };
            */
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

        Ok(())
    }
}
