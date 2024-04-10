pub struct Leaderboard {
    config:      Config,
    leaderboard: Vec<(String, u64)>,
    active:      bool,
}

fn draw_leaderboard(widget: &Dashboard, area: Rect, buf: &mut Buffer) {
    let barchart = BarChart::default()
    .block(Block::default().borders(Borders::ALL).title("Leaderboard"))
    //.data(&widget.leaderboard.iter().map(|x| (x[0], x[1].parse().unwrap())).collect::<Vec<_>>())
    .data(&widget.leaderboard)
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
