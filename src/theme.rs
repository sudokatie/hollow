//! Theme configuration for Hollow
//!
//! Provides customizable color themes for the editor.

use ratatui::style::Color;
use serde::Deserialize;

/// A color theme for the editor
#[derive(Debug, Clone, Deserialize)]
pub struct Theme {
    /// Theme name
    #[serde(default = "default_theme_name")]
    pub name: String,

    /// Background color
    #[serde(default = "default_bg")]
    pub background: ThemeColor,

    /// Main text color
    #[serde(default = "default_text")]
    pub text: ThemeColor,

    /// Dimmed/secondary text color
    #[serde(default = "default_dim")]
    pub dim: ThemeColor,

    /// Cursor color
    #[serde(default = "default_cursor")]
    pub cursor: ThemeColor,

    /// Status bar background
    #[serde(default = "default_status_bg")]
    pub status_bg: ThemeColor,

    /// Status bar text
    #[serde(default = "default_status_text")]
    pub status_text: ThemeColor,

    /// Selection/highlight color
    #[serde(default = "default_highlight")]
    pub highlight: ThemeColor,

    /// Success/positive color (e.g., goal met)
    #[serde(default = "default_success")]
    pub success: ThemeColor,

    /// Warning color
    #[serde(default = "default_warning")]
    pub warning: ThemeColor,

    /// Border color
    #[serde(default = "default_border")]
    pub border: ThemeColor,
}

/// Color representation that can be RGB or named
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ThemeColor {
    /// RGB color as [r, g, b]
    Rgb([u8; 3]),
    /// Named color (e.g., "white", "black", "reset")
    Named(String),
}

impl ThemeColor {
    /// Convert to ratatui Color
    pub fn to_color(&self) -> Color {
        match self {
            ThemeColor::Rgb([r, g, b]) => Color::Rgb(*r, *g, *b),
            ThemeColor::Named(name) => match name.to_lowercase().as_str() {
                "black" => Color::Black,
                "white" => Color::White,
                "red" => Color::Red,
                "green" => Color::Green,
                "yellow" => Color::Yellow,
                "blue" => Color::Blue,
                "magenta" => Color::Magenta,
                "cyan" => Color::Cyan,
                "gray" | "grey" => Color::Gray,
                "darkgray" | "darkgrey" => Color::DarkGray,
                "lightred" => Color::LightRed,
                "lightgreen" => Color::LightGreen,
                "lightyellow" => Color::LightYellow,
                "lightblue" => Color::LightBlue,
                "lightmagenta" => Color::LightMagenta,
                "lightcyan" => Color::LightCyan,
                "reset" | "default" => Color::Reset,
                _ => Color::Reset,
            },
        }
    }
}

// Default color functions
fn default_theme_name() -> String {
    "dark".to_string()
}

fn default_bg() -> ThemeColor {
    ThemeColor::Named("reset".to_string())
}

fn default_text() -> ThemeColor {
    ThemeColor::Named("white".to_string())
}

fn default_dim() -> ThemeColor {
    ThemeColor::Named("gray".to_string())
}

fn default_cursor() -> ThemeColor {
    ThemeColor::Named("white".to_string())
}

fn default_status_bg() -> ThemeColor {
    ThemeColor::Named("darkgray".to_string())
}

fn default_status_text() -> ThemeColor {
    ThemeColor::Named("white".to_string())
}

fn default_highlight() -> ThemeColor {
    ThemeColor::Named("yellow".to_string())
}

fn default_success() -> ThemeColor {
    ThemeColor::Named("green".to_string())
}

fn default_warning() -> ThemeColor {
    ThemeColor::Named("yellow".to_string())
}

fn default_border() -> ThemeColor {
    ThemeColor::Named("gray".to_string())
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    /// Dark theme (default)
    pub fn dark() -> Self {
        Self {
            name: "dark".to_string(),
            background: ThemeColor::Named("reset".to_string()),
            text: ThemeColor::Named("white".to_string()),
            dim: ThemeColor::Named("gray".to_string()),
            cursor: ThemeColor::Named("white".to_string()),
            status_bg: ThemeColor::Named("darkgray".to_string()),
            status_text: ThemeColor::Named("white".to_string()),
            highlight: ThemeColor::Named("yellow".to_string()),
            success: ThemeColor::Named("green".to_string()),
            warning: ThemeColor::Named("yellow".to_string()),
            border: ThemeColor::Named("gray".to_string()),
        }
    }

    /// Light theme
    pub fn light() -> Self {
        Self {
            name: "light".to_string(),
            background: ThemeColor::Rgb([255, 255, 255]),
            text: ThemeColor::Rgb([30, 30, 30]),
            dim: ThemeColor::Rgb([120, 120, 120]),
            cursor: ThemeColor::Rgb([0, 0, 0]),
            status_bg: ThemeColor::Rgb([230, 230, 230]),
            status_text: ThemeColor::Rgb([30, 30, 30]),
            highlight: ThemeColor::Rgb([255, 200, 0]),
            success: ThemeColor::Rgb([0, 150, 0]),
            warning: ThemeColor::Rgb([200, 150, 0]),
            border: ThemeColor::Rgb([180, 180, 180]),
        }
    }

    /// Sepia theme (warm, paper-like)
    pub fn sepia() -> Self {
        Self {
            name: "sepia".to_string(),
            background: ThemeColor::Rgb([250, 240, 220]),
            text: ThemeColor::Rgb([80, 60, 40]),
            dim: ThemeColor::Rgb([140, 120, 100]),
            cursor: ThemeColor::Rgb([60, 40, 20]),
            status_bg: ThemeColor::Rgb([220, 200, 170]),
            status_text: ThemeColor::Rgb([80, 60, 40]),
            highlight: ThemeColor::Rgb([255, 180, 100]),
            success: ThemeColor::Rgb([100, 140, 80]),
            warning: ThemeColor::Rgb([180, 140, 60]),
            border: ThemeColor::Rgb([180, 160, 130]),
        }
    }

    /// Solarized dark theme
    pub fn solarized() -> Self {
        Self {
            name: "solarized".to_string(),
            background: ThemeColor::Rgb([0, 43, 54]),       // base03
            text: ThemeColor::Rgb([131, 148, 150]),         // base0
            dim: ThemeColor::Rgb([88, 110, 117]),           // base01
            cursor: ThemeColor::Rgb([253, 246, 227]),       // base3
            status_bg: ThemeColor::Rgb([7, 54, 66]),        // base02
            status_text: ThemeColor::Rgb([147, 161, 161]),  // base1
            highlight: ThemeColor::Rgb([181, 137, 0]),      // yellow
            success: ThemeColor::Rgb([133, 153, 0]),        // green
            warning: ThemeColor::Rgb([203, 75, 22]),        // orange
            border: ThemeColor::Rgb([88, 110, 117]),        // base01
        }
    }

    /// Get a preset theme by name
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "dark" => Some(Self::dark()),
            "light" => Some(Self::light()),
            "sepia" => Some(Self::sepia()),
            "solarized" => Some(Self::solarized()),
            _ => None,
        }
    }

    /// List available preset themes
    pub fn presets() -> &'static [&'static str] {
        &["dark", "light", "sepia", "solarized"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dark_theme() {
        let theme = Theme::dark();
        assert_eq!(theme.name, "dark");
        assert!(matches!(theme.text.to_color(), Color::White));
    }

    #[test]
    fn test_light_theme() {
        let theme = Theme::light();
        assert_eq!(theme.name, "light");
        assert!(matches!(theme.background.to_color(), Color::Rgb(255, 255, 255)));
    }

    #[test]
    fn test_sepia_theme() {
        let theme = Theme::sepia();
        assert_eq!(theme.name, "sepia");
    }

    #[test]
    fn test_solarized_theme() {
        let theme = Theme::solarized();
        assert_eq!(theme.name, "solarized");
    }

    #[test]
    fn test_from_name() {
        assert!(Theme::from_name("dark").is_some());
        assert!(Theme::from_name("LIGHT").is_some());
        assert!(Theme::from_name("sepia").is_some());
        assert!(Theme::from_name("solarized").is_some());
        assert!(Theme::from_name("invalid").is_none());
    }

    #[test]
    fn test_presets_list() {
        let presets = Theme::presets();
        assert!(presets.contains(&"dark"));
        assert!(presets.contains(&"light"));
        assert!(presets.contains(&"sepia"));
        assert!(presets.contains(&"solarized"));
    }

    #[test]
    fn test_rgb_color() {
        let color = ThemeColor::Rgb([100, 150, 200]);
        assert!(matches!(color.to_color(), Color::Rgb(100, 150, 200)));
    }

    #[test]
    fn test_named_colors() {
        assert!(matches!(
            ThemeColor::Named("white".to_string()).to_color(),
            Color::White
        ));
        assert!(matches!(
            ThemeColor::Named("black".to_string()).to_color(),
            Color::Black
        ));
        assert!(matches!(
            ThemeColor::Named("green".to_string()).to_color(),
            Color::Green
        ));
        assert!(matches!(
            ThemeColor::Named("reset".to_string()).to_color(),
            Color::Reset
        ));
    }

    #[test]
    fn test_named_color_case_insensitive() {
        assert!(matches!(
            ThemeColor::Named("WHITE".to_string()).to_color(),
            Color::White
        ));
        assert!(matches!(
            ThemeColor::Named("DarkGray".to_string()).to_color(),
            Color::DarkGray
        ));
    }

    #[test]
    fn test_unknown_named_color_defaults_to_reset() {
        let color = ThemeColor::Named("notacolor".to_string());
        assert!(matches!(color.to_color(), Color::Reset));
    }

    #[test]
    fn test_default_theme() {
        let theme = Theme::default();
        assert_eq!(theme.name, "dark");
    }
}
