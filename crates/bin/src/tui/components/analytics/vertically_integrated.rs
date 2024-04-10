use brontes_types::mev::events::Action;
use color_eyre::eyre::Result;
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::UnboundedSender;

use crate::tui::{
    components::{Component, Frame},
    config::Config,
};

#[derive(Default, Debug)]
pub struct VerticallyIntegrated {
    command_tx: Option<UnboundedSender<Action>>,
    config:     Config,
}

impl VerticallyIntegrated {
    pub fn new() -> Self {
        Self::default()
    }

    fn draw_vertically_integrated(_widget: &VerticallyIntegrated, area: Rect, buf: &mut Buffer) {
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
                    .title("get_vertically_integrated_searchers"),
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

impl Component for VerticallyIntegrated {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn name(&self) -> String {
        "Vertically_integrated".to_string()
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
        let area = area.inner(&Margin { vertical: 1, horizontal: 4 });

        let template = Layout::default()
            .constraints([Constraint::Length(4), Constraint::Min(8), Constraint::Length(1)])
            .split(area);

        let chunks = Layout::default()
            .constraints([Constraint::Length(9), Constraint::Min(8), Constraint::Length(20)])
            .split(template[1]);

        let sub_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[1]);

        let buf = f.buffer_mut();

        Self::draw_vertically_integrated(self, sub_layout[0], buf);

        Ok(())
    }
}
