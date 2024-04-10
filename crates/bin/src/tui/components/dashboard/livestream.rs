#[derive(Default, Debug)]
pub struct Livestream {
    #[serde(default)]
    pub keybindings:           KeyBindings,
    mevblocks:                 DataFrame,
    mev_bundles:               DataFrame,
    pub popup_scroll_position: usize,
    pub popup_scroll_state:    ScrollbarState,
    pub show_popup:            bool,
}

impl Livestream {
    fn draw_livestream(widget: &mut Dashboard, area: Rect, buf: &mut Buffer) {
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

        let df = bundles_to_dataframe(mevblocks_guard.clone()).unwrap();
        let rows = dataframe_to_table_rows(&df);

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

        ratatui::widgets::StatefulWidget::render(t, area, buf, &mut widget.stream_table_state);
    }
}
