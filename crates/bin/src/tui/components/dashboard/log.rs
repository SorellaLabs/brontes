use ratatui::{
    layout::Rect,
    prelude::Buffer,
    style::{Color, Style},
    widgets::Widget,
};
use tui_logger::*;

fn draw_logs(area: Rect, buf: &mut Buffer) {
    TuiLoggerSmartWidget::default()
        .style_error(Style::default().fg(Color::Red))
        .style_debug(Style::default().fg(Color::LightRed))
        .style_warn(Style::default().fg(Color::Yellow))
        .style_trace(Style::default().fg(Color::LightMagenta))
        .style_info(Style::default().fg(Color::Cyan))
        .output_separator(':')
        .output_timestamp(Some("%H:%M:%S".to_string()))
        .output_level(Some(TuiLoggerLevelOutput::Abbreviated))
        .output_target(true)
        .title_target("Target Selector")
        .title_log("Logs")
        .render(area, buf);
}
