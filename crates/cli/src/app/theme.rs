use ratatui::style::Color;
use std::sync::OnceLock;

static THEME: OnceLock<Theme> = OnceLock::new();

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub text: Color,
    pub text_dim: Color,
    pub label: Color,
    pub highlight: Color,
    pub highlight_match: Color,
    pub error: Color,
    pub info: Color,
}

impl Theme {
    pub fn dark() -> Self {
        Theme {
            text: Color::Reset,
            text_dim: Color::Gray,
            label: Color::Cyan,
            highlight: Color::Yellow,
            highlight_match: Color::Green,
            error: Color::Red,
            info: Color::Cyan,
        }
    }

    pub fn light() -> Self {
        Theme {
            text: Color::Black,
            text_dim: Color::DarkGray,
            label: Color::Blue,
            highlight: Color::Rgb(180, 140, 0),
            highlight_match: Color::Rgb(0, 128, 0),
            error: Color::Red,
            info: Color::Blue,
        }
    }
}

pub fn detect_theme() -> Theme {
    if let Ok(fgbg) = std::env::var("COLORFGBG") {
        let parts: Vec<&str> = fgbg.split(';').collect();
        if parts.len() >= 2 {
            if let Ok(bg) = parts[parts.len() - 1].parse::<u8>() {
                let is_light = matches!(bg, 7 | 15);
                return if is_light {
                    Theme::light()
                } else {
                    Theme::dark()
                };
            }
        }
    }
    Theme::dark()
}

pub fn theme() -> &'static Theme {
    THEME.get_or_init(detect_theme)
}
