use std::sync::{Arc, Mutex};

use brontes_database::tui::events::TuiUpdate;
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{prelude::*, widgets::*, Frame};
use tokio::sync::mpsc::UnboundedSender;

use crate::tui::{
    //events::{Event, EventHandler},
    app::layout,
    config::Config,
    theme::THEME,
};

#[derive(Clone, Debug, Default)]
pub struct Navigation {}

impl Navigation {
    fn render_title_bar(&self, area: Rect, buf: &mut Buffer, page_index: usize) {
        let area = layout(area, Direction::Horizontal, vec![0, 58]);

        Paragraph::new(Span::styled("Brontes", THEME.app_title)).render(area[0], buf);
        let titles = vec!["DASHBOARD", "EXPLORER", " ANALYTICS ", " METRICS ", " SETTINGS "];
        Tabs::new(titles)
            .style(THEME.tabs)
            .highlight_style(THEME.tabs_selected)
            .select(page_index)
            .divider("")
            .render(area[1], buf);
    }

    pub fn draw(&mut self, f: &mut Frame<'_>, area: Rect, page_index: usize) {
        let area = area.inner(&Margin { vertical: 1, horizontal: 4 });

        let template = Layout::default()
            .constraints([Constraint::Length(1), Constraint::Min(8), Constraint::Length(1)])
            .split(area);
        let buf = f.buffer_mut();

        Self::render_title_bar(self, template[0], buf, page_index);
    }
}
