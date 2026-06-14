use ratatui::style::{Color, Modifier, Style};

use crate::config::ThemeName;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ThemePalette {
    pub name: ThemeName,
    pub foreground: Color,
    pub muted: Color,
    pub emphasis: Color,
    pub border: Color,
    pub surface: Color,
    pub selection_fg: Color,
    pub selection_bg: Color,
    pub accent: Color,
    pub secondary: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,
}

impl ThemePalette {
    pub fn title(self) -> Style {
        Style::new().fg(self.accent).add_modifier(Modifier::BOLD)
    }

    pub fn label(self) -> Style {
        Style::new().fg(self.emphasis).add_modifier(Modifier::BOLD)
    }

    pub fn value(self) -> Style {
        Style::new().fg(self.foreground)
    }

    pub fn muted(self) -> Style {
        Style::new().fg(self.muted)
    }

    pub fn selected(self) -> Style {
        Style::new().fg(self.selection_fg).bg(self.selection_bg)
    }

    pub fn border(self, focused: bool) -> Style {
        if focused {
            Style::new().fg(self.accent)
        } else {
            Style::new().fg(self.border)
        }
    }

    pub fn success(self) -> Style {
        Style::new().fg(self.success)
    }

    pub fn warning(self) -> Style {
        Style::new().fg(self.warning)
    }

    pub fn error(self) -> Style {
        Style::new().fg(self.error)
    }

    pub fn info(self) -> Style {
        Style::new().fg(self.info)
    }
}

pub struct ThemeRegistry;

impl ThemeRegistry {
    pub const fn all() -> [ThemeName; 3] {
        ThemeName::ALL
    }

    pub const fn get(name: ThemeName) -> ThemePalette {
        match name {
            ThemeName::TokyoNight => tokyo_night(),
            ThemeName::CatppuccinMocha => catppuccin_mocha(),
            ThemeName::GruvboxDark => gruvbox_dark(),
        }
    }
}

const fn rgb(red: u8, green: u8, blue: u8) -> Color {
    Color::Rgb(red, green, blue)
}

const fn tokyo_night() -> ThemePalette {
    ThemePalette {
        name: ThemeName::TokyoNight,
        foreground: rgb(220, 226, 250),
        muted: rgb(132, 144, 180),
        emphasis: rgb(244, 247, 255),
        border: rgb(104, 116, 160),
        surface: rgb(36, 40, 59),
        selection_fg: rgb(26, 27, 38),
        selection_bg: rgb(255, 205, 115),
        accent: rgb(255, 205, 115),
        secondary: rgb(214, 185, 255),
        success: rgb(158, 206, 106),
        warning: rgb(255, 205, 115),
        error: rgb(247, 118, 142),
        info: rgb(137, 221, 255),
    }
}

const fn catppuccin_mocha() -> ThemePalette {
    ThemePalette {
        name: ThemeName::CatppuccinMocha,
        foreground: rgb(225, 232, 255),
        muted: rgb(155, 163, 190),
        emphasis: rgb(249, 250, 255),
        border: rgb(104, 111, 145),
        surface: rgb(49, 50, 68),
        selection_fg: rgb(30, 30, 46),
        selection_bg: rgb(249, 226, 175),
        accent: rgb(249, 226, 175),
        secondary: rgb(220, 190, 255),
        success: rgb(166, 227, 161),
        warning: rgb(249, 226, 175),
        error: rgb(243, 139, 168),
        info: rgb(137, 220, 235),
    }
}

const fn gruvbox_dark() -> ThemePalette {
    ThemePalette {
        name: ThemeName::GruvboxDark,
        foreground: rgb(251, 241, 199),
        muted: rgb(180, 165, 145),
        emphasis: rgb(251, 241, 199),
        border: rgb(130, 114, 92),
        surface: rgb(60, 56, 54),
        selection_fg: rgb(40, 40, 40),
        selection_bg: rgb(250, 189, 47),
        accent: rgb(250, 189, 47),
        secondary: rgb(238, 164, 188),
        success: rgb(184, 187, 38),
        warning: rgb(250, 189, 47),
        error: rgb(251, 73, 52),
        info: rgb(142, 192, 124),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_contains_three_named_themes() {
        assert_eq!(
            ThemeRegistry::all(),
            [
                ThemeName::TokyoNight,
                ThemeName::CatppuccinMocha,
                ThemeName::GruvboxDark
            ]
        );
    }

    #[test]
    fn each_theme_has_distinct_selection_color() {
        let palettes = ThemeRegistry::all().map(ThemeRegistry::get);

        assert_ne!(palettes[0].selection_bg, palettes[1].selection_bg);
        assert_ne!(palettes[1].selection_bg, palettes[2].selection_bg);
        assert_ne!(palettes[0].selection_bg, palettes[2].selection_bg);
    }
}
