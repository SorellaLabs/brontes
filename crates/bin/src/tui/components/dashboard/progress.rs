use brontes_database::{
    tui::events::{ProgressBar, ProgressUpdate},
    Tables,
};
use brontes_types::FastHashMap;

use ratatui::{
    layout::{Constraint, Direction, Flex, Layout},
    prelude::{Buffer, Color, Modifier, Rect, Style},
    widgets::{Block, Gauge, Widget, WidgetRef},
};

#[derive(Default, Debug)]
pub struct Progress {
    pub global_progress_bar: ProgressBar,
    pub init_progress_bars:  FastHashMap<Tables, ProgressBar>,
}

impl Progress {
    fn progress_task(&mut self, update: ProgressUpdate) {
        match update {
            ProgressUpdate::Global(progress) => self.global_progress_bar = progress,
            ProgressUpdate::Table((table, progress)) => {
                self.init_progress_bars
                    .entry(table)
                    .and_modify(|e| e.position = progress.target)
                    .or_insert_with(|| progress);
            }
        }
    }

    fn render_global_progress_bar(&self, area: Rect, buf: &mut Buffer) {
        Gauge::default()
            .block(Block::bordered().title("Global Progress:"))
            .gauge_style(
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::ITALIC),
            )
            .ratio(
                self.global_progress_bar.position as f64 / self.global_progress_bar.target as f64,
            )
            .render(area, buf);
    }

    fn render_init_progress_bar(
        &self,
        area: Rect,
        buf: &mut Buffer,
        table: &Tables,
        progress: &ProgressBar,
    ) {
        Gauge::default()
            .block(Block::bordered().title(format!("{} Initialization:", table)))
            .gauge_style(
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::ITALIC),
            )
            .ratio(progress.position as f64 / progress.target as f64)
            .use_unicode(true)
            .render(area, buf);
    }

    fn create_layout(&self, area: Rect) -> Vec<Rect> {
        let mut layouts = Vec::new();

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
            .split(area);

        layouts.push(layout[0]);

        let split_init_bars = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(layout[1]);

        let first_half_size: u32 = ((self.init_progress_bars.len() + 1) / 2) as u32;
        let second_half_size: u32 = (self.init_progress_bars.len() as u32) - first_half_size;

        let constraints_first_half =
            vec![Constraint::Ratio(1, first_half_size); first_half_size as usize];
        let constraints_second_half =
            vec![Constraint::Ratio(1, second_half_size); second_half_size as usize];

        let init_progress_bars_area_first_half = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints_first_half)
            .split(split_init_bars[0]);

        layouts.extend(init_progress_bars_area_first_half.iter());

        let init_progress_bars_area_second_half = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints_second_half)
            .flex(Flex::Start)
            .split(split_init_bars[1]);

        layouts.extend(init_progress_bars_area_second_half.iter());

        layouts
    }
}

impl WidgetRef for Progress {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let layout = self.create_layout(area);

        self.render_global_progress_bar(layout[0], buf);

        for (index, (table, progress)) in self.init_progress_bars.iter().enumerate() {
            self.render_init_progress_bar(layout[index + 1], buf, table, progress);
        }
    }
}

/*let init_block = Block::bordered()
    .title("Initialization Progress")
    .title_style(Style::new().gree().bold());

let inner_block_area = init_block.inner(area); */
