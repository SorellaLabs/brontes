use std::{Debug, Default};

use ahash::FastHashMap;
use ratatui::Widget;

use crate::events::BrontesData;

#[derive(Default, Debug)]
pub struct Progress {
    pub global_progress_bar: ProgressBar,
    pub init_progress_bars:  FastHashMap<Tables, ProgressBar>,
}

#[derive(Default, Debug)]
pub struct ProgressBar {
    pub position: usize,
    pub total:    usize,
}

impl Progress {
    fn progress_task(tx: UnboundedReceiver<BrontesData>) -> Result<()> {
        if let Some(BrontesData::ProgressUpdate(update)) = tx.recv().try_recv()? {
            match update {
                ProgressUpdate::Global(block) => self.global_progress_bar = block,
                ProgressUpdate::Table(table, block) => {
                    self.init_progress_bars
                        .entry(table)
                        .and_modify(|e| e.position = block.position)
                        .or_insert_with(|| ProgressBar {
                            position: block.position,
                            total:    block.total,
                        });
                }
            }
        }
        Ok(())
    }

    fn render_progress(&self, area: Rect, buf: &mut Buffer) {
        let progress = self.progress_counter.unwrap_or(0);
        Gauge::default()
            .block(Block::bordered().title("Initialization Progress:"))
            .gauge_style((Color::Green, Modifier::ITALIC))
            .percent(progress)
            .render(area, buf);
    }
}

impl Widget for Progress {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let total_progress_bars = self.init_progress_bars.len() + 1;
        let global_progress_bar_height = (area.height as f32 * 0.3).ceil() as u16;
        let remaining_height = area.height - global_progress_bar_height;
        let sub_progress_bar_height = remaining_height / total_progress_bars as u16;

        // Render the global progress bar
        let global_progress_bar_area =
            Rect::new(area.left(), area.top(), area.width, global_progress_bar_height);
        let global_progress_percent = self.global_progress_bar.position as f32
            / self.global_progress_bar.total as f32
            * 100.0;
        Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Global Progress"),
            )
            .gauge_style(
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::ITALIC),
            )
            .percent(global_progress_percent as u16)
            .render(global_progress_bar_area, buf);

        // Render the sub progress bars
        let mut sub_progress_bar_top = global_progress_bar_area.bottom() + 1;
        for (key, progress_bar) in &self.init_progress_bars {
            let sub_progress_bar_area =
                Rect::new(area.left(), sub_progress_bar_top, area.width, sub_progress_bar_height);
            let sub_progress_percent =
                progress_bar.position as f32 / progress_bar.total as f32 * 100.0;
            Gauge::default()
                .block(Block::default().borders(Borders::ALL).title(key))
                .gauge_style(
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::ITALIC),
                )
                .percent(sub_progress_percent as u16)
                .render(sub_progress_bar_area, buf);
            sub_progress_bar_top += sub_progress_bar_height;
        }
    }
}
