use std::{Debug, Default};

use ahash::FastHashMap;

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
