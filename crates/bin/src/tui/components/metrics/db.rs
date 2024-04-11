use std::env;

use brontes_database::tui::events::TuiUpdate;
use color_eyre::eyre::Result;
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::UnboundedSender;

use super::{Component, Frame};
use crate::tui::{config::Config, tui::Tui};

#[derive(Clone, Debug, Default)]
pub struct DbSize {
    command_tx: Option<UnboundedSender<TuiUpdate>>,
    config:     Config,
}

impl DbSize {
    pub fn new() -> Self {
        Self { command_tx: Default::default(), config: Default::default() }
    }

    fn draw_dbsize(_widget: &DbSize, area: Rect, buf: &mut Buffer) {
        // Construct the final Vec<(&str, u64)> with the total counts
        let data: Vec<(&str, u64)> = vec![
            ("Sandwich", 20),
            ("Cex-Dex", 19),
            ("Jit", 15),
            ("Jit Sandwich", 10),
            ("Atomic Backrun", 5),
            ("Liquidation", 3),
        ];

        let barchart = BarChart::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("DB Size by tables"),
            )
            .data(&data)
            .bar_width(1)
            .bar_gap(0)
            .bar_set(symbols::bar::NINE_LEVELS)
            .value_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Green)
                    .add_modifier(Modifier::ITALIC),
            )
            .direction(Direction::Horizontal)
            .label_style(Style::default().fg(Color::Yellow))
            .bar_style(Style::default().fg(Color::Green));
        barchart.render(area, buf);
    }
}

impl Component for DbSize {
    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn name(&self) -> String {
        "DbSize".to_string()
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) {
        let area = area.inner(&Margin { vertical: 1, horizontal: 4 });

        let template = Layout::default()
            .constraints([Constraint::Length(5), Constraint::Min(8), Constraint::Length(1)])
            .split(area);

        let chunks = Layout::default()
            .constraints([Constraint::Length(11), Constraint::Min(8), Constraint::Length(20)])
            .split(template[1]);

        let sub_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[0]);

        let buf = f.buffer_mut();

        Self::draw_dbsize(self, sub_layout[0], buf);
    }
}
