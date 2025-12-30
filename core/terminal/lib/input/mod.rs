//! Input handling for terminal sessions.

mod keys;
mod paste;

pub use keys::{KeyInput, SpecialKey};
pub use paste::{encode_text, wrap_bracketed_paste, BracketedPasteMode};
