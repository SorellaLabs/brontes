use brontes_types::mev::Bundle;
use polars::frame::DataFrame;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::{Buffer, Color, Margin, Modifier, Rect, Style},
    widgets::{
        Block, Borders, Cell, Clear, Padding, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table, TableState,
    },
    Frame,
};

use crate::tui::{
    components::dashboard::Row,
    config::KeyBindings,
    utils::{bundles_to_dataframe, dataframe_to_table_rows},
};

//TODO: need to add Vec<Bundles> buffer to store the text string of the bundles
// that could feasilby be selected to be displayed TODO: without having to hold
// the entire bundle history in memory
#[derive(Default, Debug)]
pub struct Livestream {
    pub keybindings:           KeyBindings,
    mevblocks:                 DataFrame,
    mev_bundles:               DataFrame,
    pub popup_scroll_position: u16,
    pub popup_scroll_state:    ScrollbarState,
    pub show_popup:            bool,
    pub stream_table_state:    TableState,
}

impl Livestream {
    pub fn draw_livestream(&mut self, area: Rect, buf: &mut Buffer) {
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

    fn draw_popup(&mut self, f: &mut Frame, area: Rect) {
        todo!();
        /*
        if let Some(selected_index) = self.stream_table_state.selected() {
            let block = Block::default()
                .title("MEV Details")
                .borders(Borders::ALL)
                .padding(Padding::horizontal(4));

            let area = centered_rect(80, 80, area);
            //Self::show_popup(self,area);
            f.render_widget(Clear, area); //this clears out the background
            let paragraph = Paragraph::new("Hello, world!");
            f.render_widget(paragraph, area);

            let text = mev_blocks[selected_index].to_string().into_text();

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
        //f.render_widget(block, area);*/
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
