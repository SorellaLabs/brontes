use brontes_types::mev::{bundle::Bundle, MevType};
use crossterm::event::{KeyEvent, MouseEvent};
use eyre::ErrReport;
use ratatui::{
    prelude::{Buffer, Color, Constraint, Direction, Modifier, Rect, Style},
    symbols::bar,
    widgets::{BarChart, Block, Borders, ScrollbarState, Widget},
    Frame,
};

use super::Component;

#[derive(Default, Debug)]
pub struct MevCount {
    pub sandwich_count:       u64,
    pub cex_dex_count:        u64,
    pub jit_count:            u64,
    pub jit_sandwich_count:   u64,
    pub atomic_backrun_count: u64,
    pub liquidation_count:    u64,
    pub unkonwn_count:        u64,
    pub searcher_tx_count:    u64,
}

impl MevCount {
    pub fn update_count(&mut self, bundles: Vec<Bundle>) {
        bundles.iter().for_each(|bundle| match bundle.mev_type() {
            MevType::Sandwich => self.sandwich_count += 1,
            MevType::CexDex => self.cex_dex_count += 1,
            MevType::Jit => self.jit_count += 1,
            MevType::JitSandwich => self.jit_sandwich_count += 1,
            MevType::AtomicArb => self.atomic_backrun_count += 1,
            MevType::Liquidation => self.liquidation_count += 1,
            MevType::Unknown => self.unkonwn_count += 1,
            MevType::SearcherTx => self.searcher_tx_count += 1,
        });
    }

    pub fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        let data: Vec<(&str, u64)> = vec![
            ("Sandwich", self.sandwich_count),
            ("Cex-Dex", self.cex_dex_count),
            ("Jit", self.jit_count),
            ("Jit Sandwich", self.jit_sandwich_count),
            ("Atomic Backrun", self.atomic_backrun_count),
            ("Liquidation", self.liquidation_count),
        ];

        let barchart = BarChart::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Count of MEV Types"),
            )
            .data(&data)
            .bar_width(1)
            .bar_gap(0)
            .bar_set(bar::NINE_LEVELS)
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

    fn handle_key_events(&mut self, _key: KeyEvent) {}

    fn handle_mouse_events(&mut self, _mouse: MouseEvent) {}

    fn name(&self) -> String {
        "MevCount".to_string()
    }
}
