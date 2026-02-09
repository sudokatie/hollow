use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub editor: EditorConfig,
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub goals: GoalsConfig,
    #[serde(default)]
    pub versions: VersionConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EditorConfig {
    #[serde(default = "default_text_width")]
    pub text_width: usize,
    #[serde(default = "default_tab_width")]
    pub tab_width: usize,
    #[serde(default = "default_auto_save_seconds")]
    pub auto_save_seconds: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DisplayConfig {
    #[serde(default)]
    pub show_status: bool,
    #[serde(default = "default_status_timeout")]
    pub status_timeout: u64,
    #[serde(default = "default_line_spacing")]
    pub line_spacing: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GoalsConfig {
    #[serde(default)]
    pub daily_goal: usize,
    #[serde(default = "default_show_progress")]
    pub show_progress: bool,
    #[serde(default = "default_show_streak")]
    pub show_streak: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionConfig {
    #[serde(default = "default_versions_enabled")]
    pub enabled: bool,
    #[serde(default = "default_max_versions")]
    pub max_versions: usize,
    #[serde(default)]
    pub save_on_autosave: bool,
}

fn default_show_progress() -> bool {
    true
}

fn default_show_streak() -> bool {
    true
}

fn default_versions_enabled() -> bool {
    true
}

fn default_max_versions() -> usize {
    100
}

fn default_text_width() -> usize {
    80
}

fn default_tab_width() -> usize {
    4
}

fn default_auto_save_seconds() -> u64 {
    30
}

fn default_status_timeout() -> u64 {
    3
}

fn default_line_spacing() -> usize {
    1
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            text_width: default_text_width(),
            tab_width: default_tab_width(),
            auto_save_seconds: default_auto_save_seconds(),
        }
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            show_status: false,
            status_timeout: default_status_timeout(),
            line_spacing: default_line_spacing(),
        }
    }
}

impl Default for GoalsConfig {
    fn default() -> Self {
        Self {
            daily_goal: 0, // 0 means disabled
            show_progress: default_show_progress(),
            show_streak: default_show_streak(),
        }
    }
}

impl Default for VersionConfig {
    fn default() -> Self {
        Self {
            enabled: default_versions_enabled(),
            max_versions: default_max_versions(),
            save_on_autosave: false,
        }
    }
}

impl Config {
    /// Load configuration from ~/.config/hollow/config.toml
    /// Returns defaults if file is missing or invalid
    pub fn load() -> Self {
        let config_path = Self::config_path();

        match config_path {
            Some(path) if path.exists() => {
                match fs::read_to_string(&path) {
                    Ok(content) => {
                        match toml::from_str(&content) {
                            Ok(config) => Self::validate(config),
                            Err(_) => {
                                // Invalid TOML, use defaults
                                Self::default()
                            }
                        }
                    }
                    Err(_) => Self::default(),
                }
            }
            _ => Self::default(),
        }
    }

    /// Get the config file path
    fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("hollow").join("config.toml"))
    }

    /// Validate and clamp config values to acceptable ranges
    fn validate(mut config: Config) -> Config {
        // text_width: 20-200
        config.editor.text_width = config.editor.text_width.clamp(20, 200);

        // tab_width: 1-8
        config.editor.tab_width = config.editor.tab_width.clamp(1, 8);

        // auto_save_seconds: 0 (disabled) or 10-3600
        if config.editor.auto_save_seconds != 0 {
            config.editor.auto_save_seconds = config.editor.auto_save_seconds.clamp(10, 3600);
        }

        // status_timeout: 0-60
        config.display.status_timeout = config.display.status_timeout.clamp(0, 60);

        // line_spacing: 1-3
        config.display.line_spacing = config.display.line_spacing.clamp(1, 3);

        config
    }

    /// Apply command-line overrides
    pub fn with_overrides(mut self, width: Option<usize>, no_autosave: bool) -> Self {
        if let Some(w) = width {
            self.editor.text_width = w.clamp(20, 200);
        }
        if no_autosave {
            self.editor.auto_save_seconds = 0;
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.editor.text_width, 80);
        assert_eq!(config.editor.tab_width, 4);
        assert_eq!(config.editor.auto_save_seconds, 30);
        assert!(!config.display.show_status);
        assert_eq!(config.display.status_timeout, 3);
    }

    #[test]
    fn test_parse_valid_toml() {
        let toml = r#"
[editor]
text_width = 100
tab_width = 2
auto_save_seconds = 60

[display]
show_status = true
status_timeout = 5
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.editor.text_width, 100);
        assert_eq!(config.editor.tab_width, 2);
        assert_eq!(config.editor.auto_save_seconds, 60);
        assert!(config.display.show_status);
        assert_eq!(config.display.status_timeout, 5);
    }

    #[test]
    fn test_parse_partial_toml() {
        let toml = r#"
[editor]
text_width = 70
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.editor.text_width, 70);
        assert_eq!(config.editor.tab_width, 4); // default
        assert_eq!(config.editor.auto_save_seconds, 30); // default
    }

    #[test]
    fn test_validation_clamps_text_width() {
        let mut config = Config::default();
        config.editor.text_width = 10; // too small
        let validated = Config::validate(config);
        assert_eq!(validated.editor.text_width, 20);

        let mut config = Config::default();
        config.editor.text_width = 300; // too large
        let validated = Config::validate(config);
        assert_eq!(validated.editor.text_width, 200);
    }

    #[test]
    fn test_validation_clamps_tab_width() {
        let mut config = Config::default();
        config.editor.tab_width = 0;
        let validated = Config::validate(config);
        assert_eq!(validated.editor.tab_width, 1);

        let mut config = Config::default();
        config.editor.tab_width = 12;
        let validated = Config::validate(config);
        assert_eq!(validated.editor.tab_width, 8);
    }

    #[test]
    fn test_validation_auto_save() {
        // 0 is allowed (disabled)
        let mut config = Config::default();
        config.editor.auto_save_seconds = 0;
        let validated = Config::validate(config);
        assert_eq!(validated.editor.auto_save_seconds, 0);

        // Small values get clamped to 10
        let mut config = Config::default();
        config.editor.auto_save_seconds = 5;
        let validated = Config::validate(config);
        assert_eq!(validated.editor.auto_save_seconds, 10);

        // Large values get clamped to 3600
        let mut config = Config::default();
        config.editor.auto_save_seconds = 5000;
        let validated = Config::validate(config);
        assert_eq!(validated.editor.auto_save_seconds, 3600);
    }

    #[test]
    fn test_cli_overrides() {
        let config = Config::default().with_overrides(Some(60), false);
        assert_eq!(config.editor.text_width, 60);
        assert_eq!(config.editor.auto_save_seconds, 30);

        let config = Config::default().with_overrides(None, true);
        assert_eq!(config.editor.text_width, 80);
        assert_eq!(config.editor.auto_save_seconds, 0);
    }

    #[test]
    fn test_load_returns_defaults_when_no_file() {
        // This test relies on the config file not existing
        // In a real test we'd use a temp directory
        let config = Config::load();
        assert_eq!(config.editor.text_width, 80);
    }
}
