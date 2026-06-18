//! Color themes. A theme maps semantic roles (background, untyped, correct, error, caret, accent…)
//! to ratatui `Color`s. Themes are TOML; colors are parsed via ratatui's `Color::from_str`, which
//! accepts `#RRGGBB` hex, ANSI names (`red`, `darkgray`) and indexed colors (`42`).

use std::str::FromStr;

use anyhow::{anyhow, Result};
use ratatui::style::Color;
use serde::Deserialize;

/// A resolved theme: every role is a concrete ratatui `Color`.
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub bg: Color,
    pub untyped: Color,
    pub correct: Color,
    pub error: Color,
    pub error_bg: Color,
    pub caret: Color,
    pub accent: Color,
    pub sub: Color,
}

impl Theme {
    /// Parse a theme from TOML, falling back per-field where a value is optional.
    pub fn from_toml(s: &str) -> Result<Theme> {
        let raw: RawTheme = toml::from_str(s)?;
        let c = raw.colors;
        let error = parse_color(&c.error)?;
        let untyped = parse_color(&c.untyped)?;
        Ok(Theme {
            name: raw.name,
            bg: parse_color(&c.bg)?,
            untyped,
            correct: parse_color(&c.correct)?,
            error,
            error_bg: opt_color(c.error_bg.as_deref())?.unwrap_or(error),
            caret: parse_color(&c.caret)?,
            accent: parse_color(&c.accent)?,
            sub: opt_color(c.sub.as_deref())?.unwrap_or(untyped),
        })
    }

    /// A hard-coded theme used as the last-resort fallback so the game never fails to start.
    pub fn fallback() -> Theme {
        Theme {
            name: "Serika Dark (built-in)".to_string(),
            bg: Color::Rgb(50, 52, 55),
            untyped: Color::Rgb(100, 102, 105),
            correct: Color::Rgb(209, 208, 197),
            error: Color::Rgb(202, 71, 84),
            error_bg: Color::Rgb(126, 42, 51),
            caret: Color::Rgb(226, 183, 20),
            accent: Color::Rgb(226, 183, 20),
            sub: Color::Rgb(100, 102, 105),
        }
    }
}

fn parse_color(s: &str) -> Result<Color> {
    Color::from_str(s).map_err(|_| anyhow!("invalid color value: {s:?}"))
}

fn opt_color(s: Option<&str>) -> Result<Option<Color>> {
    s.map(parse_color).transpose()
}

#[derive(Debug, Deserialize)]
struct RawTheme {
    name: String,
    #[allow(dead_code)]
    author: Option<String>,
    colors: RawColors,
}

#[derive(Debug, Deserialize)]
struct RawColors {
    bg: String,
    untyped: String,
    correct: String,
    error: String,
    error_bg: Option<String>,
    caret: String,
    accent: String,
    sub: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_truecolor_theme() {
        let toml = r##"
            name = "Test"
            [colors]
            bg = "#000000"
            untyped = "#646669"
            correct = "#ffffff"
            error = "#ca4754"
            caret = "#e2b714"
            accent = "#e2b714"
        "##;
        let t = Theme::from_toml(toml).unwrap();
        assert_eq!(t.name, "Test");
        assert_eq!(t.bg, Color::Rgb(0, 0, 0));
        assert_eq!(t.correct, Color::Rgb(255, 255, 255));
        // optional fields fall back: error_bg → error, sub → untyped.
        assert_eq!(t.error_bg, t.error);
        assert_eq!(t.sub, t.untyped);
    }

    #[test]
    fn invalid_color_is_an_error_not_a_panic() {
        let toml = r##"
            name = "Bad"
            [colors]
            bg = "not-a-color"
            untyped = "#646669"
            correct = "#ffffff"
            error = "#ca4754"
            caret = "#e2b714"
            accent = "#e2b714"
        "##;
        assert!(Theme::from_toml(toml).is_err());
    }

    #[test]
    fn fallback_always_available() {
        let t = Theme::fallback();
        assert_eq!(t.bg, Color::Rgb(50, 52, 55));
    }
}
