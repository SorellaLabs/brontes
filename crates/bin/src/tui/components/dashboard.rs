impl Dashboard {
    pub fn new() -> Self {
        Self::default()
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

    #[allow(unused_variables)]
    fn draw_charts(widget: &mut Dashboard, area: Rect, buf: &mut Buffer) {
        // Initialize counters
        let mut sandwich_total = 0;
        let mut cex_dex_total = 0;
        let mut jit_total = 0;
        let mut jit_sandwich_total = 0;
        let mut atomic_backrun_total = 0;
        let mut liquidation_total = 0;

        let mevblocks_guard: std::sync::MutexGuard<'_, Vec<MevBlock>> =
            widget.mevblocks.lock().unwrap();

        // Aggregate counts
        for item in mevblocks_guard.iter() {
            sandwich_total += item.mev_count.sandwich_count.unwrap_or(0);
            cex_dex_total += item.mev_count.cex_dex_count.unwrap_or(0);
            jit_total += item.mev_count.jit_count.unwrap_or(0);
            jit_sandwich_total += item.mev_count.jit_sandwich_count.unwrap_or(0);
            atomic_backrun_total += item.mev_count.atomic_backrun_count.unwrap_or(0);
            liquidation_total += item.mev_count.liquidation_count.unwrap_or(0);
        }

        // Construct the final Vec<(&str, u64)> with the total counts
        let data: Vec<(&str, u64)> = vec![
            ("Sandwich", sandwich_total),
            ("Cex-Dex", cex_dex_total),
            ("Jit", jit_total),
            ("Jit Sandwich", jit_sandwich_total),
            ("Atomic Backrun", atomic_backrun_total),
            ("Liquidation", liquidation_total),
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

    fn render_progress(&self, area: Rect, buf: &mut Buffer) {
        let progress = self.progress_counter.unwrap_or(0);
        Gauge::default()
            .block(Block::bordered().title("PROGRESS:"))
            .gauge_style((Color::White, Modifier::ITALIC))
            .percent(progress)
            .render(area, buf);
    }
}
