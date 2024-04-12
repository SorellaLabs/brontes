mod leaderboard;
mod livestream;

use ratatui::text::Line;
mod log;
mod mev_count;
pub mod progress;

use brontes_database::tui::events::TuiUpdate;
use crossterm::event::KeyEvent;
use eyre::Result;
//
use itertools::Itertools;
use livestream::Livestream;
use mev_count::MevCount;
use polars::prelude::*;
use progress::Progress;
use ratatui::{
    layout::Layout,
    prelude::{Alignment, Buffer, Color, Constraint, Direction, Margin, Rect, Span},
    style::Stylize,
    widgets::{Cell, Paragraph, Row, Widget},
};

//TODO: Basic livestream display
//TODO: Input & data handlers for the dashboard
// TODO: Filter + sort + search functionality on the livestream framme
use super::{shared::navigation::Navigation, Component, Frame};
use crate::tui::{config::Config, theme::THEME};
#[derive(Debug)]
pub struct Dashboard {
    config:     Config,
    navigation: Navigation,
    mev_count:  MevCount,
    livestream: Livestream,
    //leaderboard: Leaderboard,
    progress:   Progress,
    focus:      Focus,
}

pub const DASHBOARD_INDEX: usize = 0;

impl Dashboard {
    pub fn new() -> Self {
        Self {
            config:     Config::default(),
            navigation: Navigation::default(),
            mev_count:  MevCount::default(),
            livestream: Livestream::default(),
            //leaderboard: Leaderboard::new(),
            progress:   Progress::default(),
            focus:      Focus::Livestream,
        }
    }

    /*pub fn next(&mut self) {
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
    }*/

    // Function to convert a DataFrame to a Vec<Row> for the Table widget
    fn dataframe_to_table_rows(df: &DataFrame) -> Vec<Row> {
        let height = 1;
        let bottom_margin = 0;

        let num_rows = df.height();
        let mut rows = Vec::with_capacity(num_rows);

        for i in 0..num_rows {
            let mut cells = Vec::new();
            for series in df.get_columns() {
                let value_str = series.get(i).unwrap().to_string();
                cells.push(Cell::from(value_str));
            }
            rows.push(Row::new(cells).height(height).bottom_margin(bottom_margin));
        }

        rows
    }
}

impl Component for Dashboard {
    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn name(&self) -> String {
        "Dashboard".to_string()
    }



    fn handle_key_events(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
             => (),
        };
        Ok(())
    }

    fn handle_data_events(&mut self, event: TuiUpdate) -> Result<()> {
        match event {
            TuiUpdate::Block((block, bundles)) => self.mev_count.update_count(bundles),

            _ => (),
        };
        Ok(())
    }

    fn on_select(&mut self, f: &mut Frame<'_>) {
        let page_index = DASHBOARD_INDEX;
        self.navigation.draw(f, f.size(), DASHBOARD_INDEX);
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) {
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

        self.mev_count.draw(sub_layout[0], buf);
        self.progress.render(template[2], buf);
        //Self::draw_leaderboard(self, sub_layout[1], buf);
        //Self::draw_logs(self, chunks[2], buf);
        self.livestream.draw_livestream(chunks[1], buf);

        Self::render_bottom_bar(self, template[3], buf);
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

impl Focus {
    fn render_command_bar(&self, area: Rect, buf: &mut Buffer) {
        let keys = match self {
            Focus::Livestream => [
                ("Q/Esc", "Quit"),
                ("Tab", "Next Tab"),
                ("BackTab", "Previous Tab"),
                ("k", "Up"),
                ("j", "Down"),
                ("l", "Right"),
                ("h", "Left"),
                ("↵", "Select Livestream"),
            ],
            _ => [
                ("Q/Esc", "Quit"),
                ("Tab", "Next Tab"),
                ("BackTab", "Previous Tab"),
                ("k", "Up"),
                ("j", "Down"),
                ("l", "Right"),
                ("h", "Left"),
                ("↵", "Select"),
            ],
        };

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

    pub fn right(&self) -> Self {
        match self {
            Focus::Livestream => Focus::Progress,
            Focus::Progress => Focus::Logs,
            Focus::Logs => Focus::MevCount,
            Focus::MevCount => Focus::LeaderBoard,
            Focus::LeaderBoard => Focus::Livestream,
        }
    }

    pub fn left(&self) -> Self {
        match self {
            Focus::Progress => Focus::Livestream,
            Focus::Logs => Focus::Progress,
            Focus::MevCount => Focus::Logs,
            Focus::LeaderBoard => Focus::MevCount,
            Focus::Livestream => Focus::Livestream,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum Focus {
    #[default]
    Livestream,
    Progress,
    Logs,
    MevCount,
    LeaderBoard,
}
