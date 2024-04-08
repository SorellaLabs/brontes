use ratatui::prelude::*;

pub struct Theme {
    pub root:              Style,
    pub content:           Style,
    pub app_title:         Style,
    pub tabs:              Style,
    pub tabs_selected:     Style,
    pub borders:           Style,
    pub description:       Style,
    pub description_title: Style,
    pub key_binding:       KeyBinding,
}

pub struct KeyBinding {
    pub key:         Style,
    pub description: Style,
}

pub const THEME: Theme = Theme {
    root:              Style::new().bg(DARK_BLUE),
    content:           Style::new().bg(DARK_BLUE).fg(LIGHT_GRAY),
    app_title:         Style::new()
        .fg(WHITE)
        .bg(DARK_BLUE)
        .add_modifier(Modifier::BOLD),
    tabs:              Style::new().fg(MID_GRAY).bg(DARK_BLUE),
    tabs_selected:     Style::new()
        .fg(WHITE)
        .bg(DARK_BLUE)
        .add_modifier(Modifier::BOLD)
        .add_modifier(Modifier::REVERSED),
    borders:           Style::new().fg(LIGHT_GRAY),
    description:       Style::new().fg(LIGHT_GRAY).bg(DARK_BLUE),
    description_title: Style::new().fg(LIGHT_GRAY).add_modifier(Modifier::BOLD),

    key_binding: KeyBinding {
        key:         Style::new().fg(BLACK).bg(DARK_GRAY),
        description: Style::new().fg(DARK_GRAY).bg(BLACK),
    },
};

const DARK_BLUE: Color = Color::Rgb(16, 24, 48);


const WHITE: Color = Color::Indexed(255); // not really white, often #eeeeee

// Not used currently, leaving for reference.
//const LIGHT_BLUE: Color = Color::Rgb(64, 96, 192);
//const LIGHT_YELLOW: Color = Color::Rgb(192, 192, 96);
//const LIGHT_GREEN: Color = Color::Rgb(64, 192, 96);
//const LIGHT_RED: Color = Color::Rgb(192, 96, 96);
//const RED: Color = Color::Indexed(160);
const BLACK: Color = Color::Indexed(232); // not really black, often #080808
const DARK_GRAY: Color = Color::Indexed(238);
const MID_GRAY: Color = Color::Indexed(244);
const LIGHT_GRAY: Color = Color::Indexed(250);