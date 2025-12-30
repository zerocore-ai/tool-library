//! Bracketed paste mode support.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Bracketed paste mode options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum BracketedPasteMode {
    /// Always use bracketed paste.
    Always,

    /// Never use bracketed paste.
    Never,

    /// Use bracketed paste for multi-line text (default).
    #[default]
    Auto,
}

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// Start sequence for bracketed paste.
pub const PASTE_START: &[u8] = b"\x1b[200~";

/// End sequence for bracketed paste.
pub const PASTE_END: &[u8] = b"\x1b[201~";

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Wrap text in bracketed paste sequences.
pub fn wrap_bracketed_paste(text: &str) -> Vec<u8> {
    let mut result = Vec::with_capacity(PASTE_START.len() + text.len() + PASTE_END.len());
    result.extend_from_slice(PASTE_START);
    result.extend_from_slice(text.as_bytes());
    result.extend_from_slice(PASTE_END);
    result
}

/// Check if bracketed paste should be used for the given text.
pub fn should_use_bracketed_paste(text: &str, mode: BracketedPasteMode) -> bool {
    match mode {
        BracketedPasteMode::Always => true,
        BracketedPasteMode::Never => false,
        BracketedPasteMode::Auto => text.contains('\n'),
    }
}

/// Encode text for sending, optionally using bracketed paste.
pub fn encode_text(text: &str, mode: BracketedPasteMode) -> Vec<u8> {
    if should_use_bracketed_paste(text, mode) {
        wrap_bracketed_paste(text)
    } else {
        text.as_bytes().to_vec()
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_bracketed_paste() {
        let wrapped = wrap_bracketed_paste("hello");
        assert!(wrapped.starts_with(PASTE_START));
        assert!(wrapped.ends_with(PASTE_END));
        assert!(wrapped.windows(5).any(|w| w == b"hello"));
    }

    #[test]
    fn test_should_use_auto_single_line() {
        assert!(!should_use_bracketed_paste("hello", BracketedPasteMode::Auto));
    }

    #[test]
    fn test_should_use_auto_multi_line() {
        assert!(should_use_bracketed_paste("hello\nworld", BracketedPasteMode::Auto));
    }

    #[test]
    fn test_should_use_always() {
        assert!(should_use_bracketed_paste("hello", BracketedPasteMode::Always));
    }

    #[test]
    fn test_should_use_never() {
        assert!(!should_use_bracketed_paste("hello\nworld", BracketedPasteMode::Never));
    }

    #[test]
    fn test_encode_text_single_line() {
        let encoded = encode_text("hello", BracketedPasteMode::Auto);
        assert_eq!(encoded, b"hello");
    }

    #[test]
    fn test_encode_text_multi_line() {
        let encoded = encode_text("hello\nworld", BracketedPasteMode::Auto);
        assert!(encoded.starts_with(PASTE_START));
    }
}
