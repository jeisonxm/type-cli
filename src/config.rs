//! Configuration: resolve XDG directories, load `config.toml` and the selected theme, and fall back
//! safely at every step. Embedded defaults mean the game runs with no files present; files on disk
//! are overrides only. Parsing never panics — a broken file degrades to defaults with a warning.

use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::Deserialize;

use crate::engine::Mode;
use crate::ui::theme::Theme;

/// Embedded defaults (single source of truth, bundled into the binary).
const DEFAULT_CONFIG: &str = include_str!("../assets/default_config.toml");
const THEME_TERMINAL: &str = include_str!("../assets/themes/terminal.toml");
const THEME_SERIKA: &str = include_str!("../assets/themes/serika_dark.toml");
const THEME_DRACULA: &str = include_str!("../assets/themes/dracula.toml");
const THEME_CLASSIC16: &str = include_str!("../assets/themes/classic16.toml");

/// Look up a built-in theme's TOML by name.
fn builtin_theme(name: &str) -> Option<&'static str> {
    match name {
        "terminal" => Some(THEME_TERMINAL),
        "serika_dark" => Some(THEME_SERIKA),
        "dracula" => Some(THEME_DRACULA),
        "classic16" => Some(THEME_CLASSIC16),
        _ => None,
    }
}

/// The fully resolved configuration handed to the app.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub settings: Settings,
    pub theme: Theme,
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
}

impl AppConfig {
    /// Load configuration from disk (creating defaults on first run), never panicking.
    pub fn load() -> Self {
        let (config_dir, data_dir) = resolve_dirs();
        let settings = load_settings(&config_dir);
        let theme = load_theme(&config_dir, &settings.appearance.theme);
        AppConfig {
            settings,
            theme,
            config_dir,
            data_dir,
        }
    }

    /// Override the active theme by name (e.g. from a `--theme` flag).
    pub fn with_theme(mut self, name: &str) -> Self {
        self.theme = load_theme(&self.config_dir, name);
        self.settings.appearance.theme = name.to_string();
        self
    }

    /// Path to the SQLite database (Phase 2): explicit override, else `<data_dir>/type-cli.db`.
    pub fn database_path(&self) -> PathBuf {
        if self.settings.paths.database.is_empty() {
            self.data_dir.join("type-cli.db")
        } else {
            PathBuf::from(&self.settings.paths.database)
        }
    }
}

/// Resolve (config_dir, data_dir). Honors `TYPE_CLI_CONFIG_HOME` for testing/portability.
fn resolve_dirs() -> (PathBuf, PathBuf) {
    if let Ok(base) = std::env::var("TYPE_CLI_CONFIG_HOME") {
        let base = PathBuf::from(base);
        return (base.join("config"), base.join("data"));
    }
    if let Some(pd) = ProjectDirs::from("com", "jeisonxm", "type-cli") {
        (pd.config_dir().to_path_buf(), pd.data_dir().to_path_buf())
    } else {
        (
            PathBuf::from(".type-cli/config"),
            PathBuf::from(".type-cli/data"),
        )
    }
}

/// Load `config.toml`, writing the embedded default on first run. Falls back to defaults on any error.
fn load_settings(config_dir: &Path) -> Settings {
    let path = config_dir.join("config.toml");
    if path.exists() {
        match std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|s| toml::from_str::<Settings>(&s).map_err(|e| e.to_string()))
        {
            Ok(s) => return s,
            Err(e) => {
                eprintln!(
                    "type-cli: could not parse {} ({e}); using defaults",
                    path.display()
                );
                return Settings::embedded_default();
            }
        }
    }
    // First run: best-effort scaffold of the default file.
    let _ = std::fs::create_dir_all(config_dir);
    if let Err(e) = std::fs::write(&path, DEFAULT_CONFIG) {
        eprintln!(
            "type-cli: could not write default config to {} ({e})",
            path.display()
        );
    }
    Settings::embedded_default()
}

/// Load a theme by name: user file override → built-in → embedded fallback. Never panics.
fn load_theme(config_dir: &Path, name: &str) -> Theme {
    let user_path = config_dir.join("themes").join(format!("{name}.toml"));
    if user_path.exists() {
        match std::fs::read_to_string(&user_path) {
            Ok(s) => match Theme::from_toml(&s) {
                Ok(t) => return t,
                Err(e) => eprintln!(
                    "type-cli: bad theme {} ({e}); falling back",
                    user_path.display()
                ),
            },
            Err(e) => eprintln!(
                "type-cli: cannot read theme {} ({e}); falling back",
                user_path.display()
            ),
        }
    }
    if let Some(toml) = builtin_theme(name) {
        if let Ok(t) = Theme::from_toml(toml) {
            return t;
        }
    }
    if let Ok(t) = Theme::from_toml(THEME_TERMINAL) {
        return t;
    }
    Theme::fallback()
}

// --- settings schema --------------------------------------------------------

/// The parsed `config.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub appearance: Appearance,
    #[serde(default)]
    pub game: GameCfg,
    #[serde(default = "default_presets")]
    pub presets: Vec<Preset>,
    #[serde(default)]
    pub paths: Paths,
}

impl Settings {
    /// Parse the embedded default config (guaranteed valid; verified by a test).
    pub fn embedded_default() -> Settings {
        toml::from_str(DEFAULT_CONFIG).expect("embedded default config must be valid")
    }

    /// Resolve a preset id to an engine `Mode`.
    pub fn mode_for(&self, id: &str) -> Option<Mode> {
        self.presets.iter().find(|p| p.id == id).map(Preset::mode)
    }

    /// The mode for the configured default preset (falls back to 60s timed).
    pub fn default_mode(&self) -> Mode {
        self.mode_for(&self.game.default_preset)
            .unwrap_or(Mode::Time { secs: 60 })
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings::embedded_default()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Appearance {
    /// Color theme name (built-in or in <config_dir>/themes/). Default blends with the terminal.
    pub theme: String,
    /// Whether the (small, discreet) timer is visible on startup. Toggle at runtime with Ctrl+T.
    pub show_timer: bool,
}

impl Default for Appearance {
    fn default() -> Self {
        Appearance {
            theme: "terminal".to_string(),
            show_timer: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct GameCfg {
    pub default_preset: String,
    pub language: String,
    pub quick_restart: String,
    pub fold_smart_punctuation: bool,
}

impl Default for GameCfg {
    fn default() -> Self {
        GameCfg {
            default_preset: "time_60".to_string(),
            language: "english".to_string(),
            quick_restart: "tab".to_string(),
            fold_smart_punctuation: true,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Paths {
    pub database: String,
}

/// An editable test preset (e.g. `time_60`, `words_100`).
#[derive(Debug, Clone, Deserialize)]
pub struct Preset {
    pub id: String,
    pub kind: PresetKind,
    pub value: u64,
}

impl Preset {
    pub fn mode(&self) -> Mode {
        match self.kind {
            PresetKind::Time => Mode::Time { secs: self.value },
            PresetKind::Words => Mode::Words {
                count: self.value as usize,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PresetKind {
    Time,
    Words,
}

fn default_schema_version() -> u32 {
    2
}

fn default_presets() -> Vec<Preset> {
    Settings::embedded_default().presets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_default_config_is_valid() {
        let s = Settings::embedded_default();
        assert_eq!(s.schema_version, 2);
        assert_eq!(s.presets.len(), 5);
        assert_eq!(s.appearance.theme, "terminal");
        assert!(!s.appearance.show_timer); // hidden by default (stealth)
    }

    #[test]
    fn all_builtin_themes_parse() {
        for name in ["terminal", "serika_dark", "dracula", "classic16"] {
            let toml = builtin_theme(name).unwrap();
            assert!(
                Theme::from_toml(toml).is_ok(),
                "theme {name} failed to parse"
            );
        }
    }

    #[test]
    fn default_preset_resolves_to_a_mode() {
        let s = Settings::embedded_default();
        assert_eq!(s.default_mode(), Mode::Time { secs: 60 });
        assert_eq!(s.mode_for("words_100"), Some(Mode::Words { count: 100 }));
        assert_eq!(s.mode_for("nope"), None);
    }

    #[test]
    fn partial_config_fills_missing_sections_with_defaults() {
        // Only an appearance theme is given; everything else must default.
        let toml = "[appearance]\ntheme = \"dracula\"\n";
        let s: Settings = toml::from_str(toml).unwrap();
        assert_eq!(s.appearance.theme, "dracula");
        assert!(!s.appearance.show_timer); // default
        assert_eq!(s.game.language, "english"); // default
        assert_eq!(s.presets.len(), 5); // default presets
    }
}
