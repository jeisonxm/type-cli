//! Figlet/ASCII-art banners for the timer, WPM and results numbers.
//!
//! Uses figlet-rs's built-in "standard" font. Generation is cheap (a few characters) and falls back
//! to the plain string if the font is unavailable, so a banner can never crash a frame.

use figlet_rs::FIGfont;

/// Render `text` as big ASCII art, or return it unchanged if figlet is unavailable.
pub fn big_text(text: &str) -> String {
    match FIGfont::standard() {
        Ok(font) => font
            .convert(text)
            .map(|fig| fig.to_string())
            .unwrap_or_else(|| text.to_string()),
        Err(_) => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_multiline_art_for_a_number() {
        let art = big_text("60");
        // figlet output spans multiple rows and is wider than the input.
        assert!(art.lines().count() > 1);
        assert!(art.len() > 2);
    }
}
