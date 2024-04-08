use crossterm::event::KeyCode;
use ratatui::style::{Color, Modifier, Style};

pub trait PrintableKeyCode {
    fn to_string(&self) -> String;
}

impl PrintableKeyCode for KeyCode {
    fn to_string(&self) -> String {
        match self {
            KeyCode::Char(c) => format!("{}", c),
            KeyCode::Enter => "Enter".to_string(),
            KeyCode::Esc => "Esc".to_string(),
            KeyCode::Backspace => "Backspace".to_string(),
            KeyCode::Left => "Left".to_string(),
            KeyCode::Right => "Right".to_string(),
            KeyCode::Up => "Up".to_string(),
            KeyCode::Down => "Down".to_string(),
            KeyCode::Home => "Home".to_string(),
            KeyCode::End => "End".to_string(),
            KeyCode::PageUp => "PageUp".to_string(),
            KeyCode::PageDown => "PageDown".to_string(),
            KeyCode::Tab => "Tab".to_string(),
            KeyCode::BackTab => "BackTab".to_string(),
            KeyCode::Delete => "Delete".to_string(),
            KeyCode::Insert => "Insert".to_string(),
            KeyCode::F(u) => format!("F{}", u),
            KeyCode::Null => "Null".to_string(),
            KeyCode::Modifier(c) => format!("{:?}", c),
            KeyCode::CapsLock => "CapsLock".to_string(),
            KeyCode::ScrollLock => "ScrollLock".to_string(),
            KeyCode::NumLock => "NumLock".to_string(),
            KeyCode::PrintScreen => "PrintScreen".to_string(),
            KeyCode::Pause => "Pause".to_string(),
            KeyCode::Menu => "Menu".to_string(),
            KeyCode::KeypadBegin => "KeypadBegin".to_string(),
            KeyCode::Media(_) => "Media".to_string(),
        }
    }
}

const DEFAULT_STYLE: Style = Style {
    fg:              None,
    bg:              None,
    underline_color: None,
    add_modifier:    Modifier::empty(),
    sub_modifier:    Modifier::empty(),
};

pub struct UiStyle;

impl UiStyle {
    pub const DEFAULT: Style = DEFAULT_STYLE;
    pub const DISCONNECTED: Style = DEFAULT_STYLE.fg(Color::DarkGray);
    pub const ERROR: Style = DEFAULT_STYLE.fg(Color::Red);
    pub const FANCY: Style = DEFAULT_STYLE.fg(Color::Rgb(244, 255, 232));
    pub const HEADER: Style = DEFAULT_STYLE.fg(Color::LightBlue);
    pub const HIGHLIGHT: Style = DEFAULT_STYLE.fg(Color::Rgb(118, 213, 192));
    pub const NETWORK: Style = DEFAULT_STYLE.fg(Color::Rgb(244, 123, 123));
    pub const OK: Style = DEFAULT_STYLE.fg(Color::Green);
    pub const OWN_TEAM: Style = DEFAULT_STYLE.fg(Color::Green);
    pub const SELECTED: Style = DEFAULT_STYLE.fg(Color::Black).bg(Color::Rgb(244, 255, 232));
    pub const UNSELECTABLE: Style = DEFAULT_STYLE.fg(Color::DarkGray);
    pub const UNSELECTED: Style = DEFAULT_STYLE;
    pub const WARNING: Style = DEFAULT_STYLE.fg(Color::Yellow);
}

pub struct UiText;

impl UiText {
    pub const NO: &'static str = "No";
    pub const YES: &'static str = "Yes";
}
