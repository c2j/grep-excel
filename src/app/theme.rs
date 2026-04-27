use ratatui::style::Color;
use std::sync::OnceLock;

static THEME: OnceLock<Theme> = OnceLock::new();

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub is_light: bool,
    pub text: Color,
    pub text_dim: Color,
    pub label: Color,
    pub border_inactive: Color,
    pub border_active: Color,
    pub highlight: Color,
    pub highlight_match: Color,
    pub error: Color,
    pub warning: Color,
    pub success: Color,
    pub info: Color,
    pub background_popup: Color,
}

impl Theme {
    pub fn dark() -> Self {
        Theme {
            is_light: false,
            text: Color::Reset,
            text_dim: Color::Gray,
            label: Color::Cyan,
            border_inactive: Color::DarkGray,
            border_active: Color::Yellow,
            highlight: Color::Yellow,
            highlight_match: Color::Green,
            error: Color::Red,
            warning: Color::Yellow,
            success: Color::Green,
            info: Color::Cyan,
            background_popup: Color::Black,
        }
    }

    pub fn light() -> Self {
        Theme {
            is_light: true,
            text: Color::Black,
            text_dim: Color::DarkGray,
            label: Color::Blue,
            border_inactive: Color::Gray,
            border_active: Color::Rgb(180, 140, 0),
            highlight: Color::Rgb(180, 140, 0),
            highlight_match: Color::Rgb(0, 128, 0),
            error: Color::Red,
            warning: Color::Rgb(180, 140, 0),
            success: Color::Rgb(0, 128, 0),
            info: Color::Blue,
            background_popup: Color::Rgb(245, 245, 245),
        }
    }
}

pub fn detect_theme() -> Theme {
    if let Some(fgbg) = std::env::var("COLORFGBG").ok() {
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

pub fn set_theme(t: Theme) {
    let _ = THEME.set(t);
}
