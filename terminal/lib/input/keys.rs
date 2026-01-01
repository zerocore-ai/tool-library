//! Special key encoding.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::types::{Result, TerminalError};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Special keys that can be sent to the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SpecialKey {
    // Navigation
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,

    // Editing
    Backspace,
    Delete,
    Insert,
    Tab,

    // Control
    Enter,
    Escape,

    // Function keys
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
}

/// Key input with modifiers.
#[derive(Debug, Clone, Default)]
pub struct KeyInput {
    /// Special key (if any).
    pub key: Option<SpecialKey>,

    /// Text to send (if any).
    pub text: Option<String>,

    /// Ctrl modifier.
    pub ctrl: bool,

    /// Alt modifier.
    pub alt: bool,

    /// Shift modifier.
    pub shift: bool,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl SpecialKey {
    /// Get the base escape sequence for this key (xterm-style).
    pub fn base_sequence(&self) -> &'static [u8] {
        match self {
            Self::Up => b"\x1b[A",
            Self::Down => b"\x1b[B",
            Self::Right => b"\x1b[C",
            Self::Left => b"\x1b[D",
            Self::Home => b"\x1b[H",
            Self::End => b"\x1b[F",
            Self::PageUp => b"\x1b[5~",
            Self::PageDown => b"\x1b[6~",
            Self::Insert => b"\x1b[2~",
            Self::Delete => b"\x1b[3~",
            Self::Backspace => b"\x7f",
            Self::Tab => b"\t",
            Self::Enter => b"\r",
            Self::Escape => b"\x1b",
            Self::F1 => b"\x1bOP",
            Self::F2 => b"\x1bOQ",
            Self::F3 => b"\x1bOR",
            Self::F4 => b"\x1bOS",
            Self::F5 => b"\x1b[15~",
            Self::F6 => b"\x1b[17~",
            Self::F7 => b"\x1b[18~",
            Self::F8 => b"\x1b[19~",
            Self::F9 => b"\x1b[20~",
            Self::F10 => b"\x1b[21~",
            Self::F11 => b"\x1b[23~",
            Self::F12 => b"\x1b[24~",
        }
    }

    /// Check if this key supports modifier encoding.
    pub fn supports_modifiers(&self) -> bool {
        matches!(
            self,
            Self::Up
                | Self::Down
                | Self::Left
                | Self::Right
                | Self::Home
                | Self::End
                | Self::PageUp
                | Self::PageDown
                | Self::Insert
                | Self::Delete
                | Self::F1
                | Self::F2
                | Self::F3
                | Self::F4
                | Self::F5
                | Self::F6
                | Self::F7
                | Self::F8
                | Self::F9
                | Self::F10
                | Self::F11
                | Self::F12
        )
    }

    /// Parse a key name string.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "up" => Some(Self::Up),
            "down" => Some(Self::Down),
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "home" => Some(Self::Home),
            "end" => Some(Self::End),
            "pageup" | "page_up" => Some(Self::PageUp),
            "pagedown" | "page_down" => Some(Self::PageDown),
            "backspace" => Some(Self::Backspace),
            "delete" | "del" => Some(Self::Delete),
            "insert" | "ins" => Some(Self::Insert),
            "tab" => Some(Self::Tab),
            "enter" | "return" => Some(Self::Enter),
            "escape" | "esc" => Some(Self::Escape),
            "f1" => Some(Self::F1),
            "f2" => Some(Self::F2),
            "f3" => Some(Self::F3),
            "f4" => Some(Self::F4),
            "f5" => Some(Self::F5),
            "f6" => Some(Self::F6),
            "f7" => Some(Self::F7),
            "f8" => Some(Self::F8),
            "f9" => Some(Self::F9),
            "f10" => Some(Self::F10),
            "f11" => Some(Self::F11),
            "f12" => Some(Self::F12),
            _ => None,
        }
    }
}

impl KeyInput {
    /// Encode the key input to bytes for the PTY.
    pub fn encode(&self) -> Result<Vec<u8>> {
        // Handle Ctrl+letter
        if self.ctrl && !self.alt && self.key.is_none() {
            if let Some(ref text) = self.text {
                if text.len() == 1 {
                    let c = text.chars().next().unwrap();
                    if c.is_ascii_alphabetic() {
                        // Ctrl+A = 1, Ctrl+B = 2, ..., Ctrl+Z = 26
                        let ctrl_code = (c.to_ascii_uppercase() as u8) - b'A' + 1;
                        return Ok(vec![ctrl_code]);
                    }
                }
            }
        }

        // Handle special keys
        if let Some(key) = self.key {
            return self.encode_special_key(key);
        }

        // Handle text
        if let Some(ref text) = self.text {
            let mut result = Vec::new();

            for c in text.chars() {
                if self.alt {
                    // Alt + char = ESC + char
                    result.push(0x1b);
                }

                if self.ctrl && c.is_ascii_alphabetic() {
                    let ctrl_code = (c.to_ascii_uppercase() as u8) - b'A' + 1;
                    result.push(ctrl_code);
                } else {
                    let mut buf = [0u8; 4];
                    let encoded = c.encode_utf8(&mut buf);
                    result.extend_from_slice(encoded.as_bytes());
                }
            }

            return Ok(result);
        }

        Err(TerminalError::NoInput)
    }

    /// Encode a special key with modifiers.
    fn encode_special_key(&self, key: SpecialKey) -> Result<Vec<u8>> {
        let modifier_code = self.modifier_code();

        // No modifiers, use base sequence
        if modifier_code == 1 {
            return Ok(key.base_sequence().to_vec());
        }

        // With modifiers, need to modify the sequence
        if !key.supports_modifiers() {
            // For keys that don't support modifiers, just use base
            return Ok(key.base_sequence().to_vec());
        }

        // Build modified sequence
        match key {
            // Arrow keys and Home/End: \x1b[1;{mod}X
            SpecialKey::Up => Ok(format!("\x1b[1;{}A", modifier_code).into_bytes()),
            SpecialKey::Down => Ok(format!("\x1b[1;{}B", modifier_code).into_bytes()),
            SpecialKey::Right => Ok(format!("\x1b[1;{}C", modifier_code).into_bytes()),
            SpecialKey::Left => Ok(format!("\x1b[1;{}D", modifier_code).into_bytes()),
            SpecialKey::Home => Ok(format!("\x1b[1;{}H", modifier_code).into_bytes()),
            SpecialKey::End => Ok(format!("\x1b[1;{}F", modifier_code).into_bytes()),

            // Keys with ~ terminator: \x1b[N;{mod}~
            SpecialKey::PageUp => Ok(format!("\x1b[5;{}~", modifier_code).into_bytes()),
            SpecialKey::PageDown => Ok(format!("\x1b[6;{}~", modifier_code).into_bytes()),
            SpecialKey::Insert => Ok(format!("\x1b[2;{}~", modifier_code).into_bytes()),
            SpecialKey::Delete => Ok(format!("\x1b[3;{}~", modifier_code).into_bytes()),

            // F1-F4: \x1b[1;{mod}P/Q/R/S
            SpecialKey::F1 => Ok(format!("\x1b[1;{}P", modifier_code).into_bytes()),
            SpecialKey::F2 => Ok(format!("\x1b[1;{}Q", modifier_code).into_bytes()),
            SpecialKey::F3 => Ok(format!("\x1b[1;{}R", modifier_code).into_bytes()),
            SpecialKey::F4 => Ok(format!("\x1b[1;{}S", modifier_code).into_bytes()),

            // F5-F12: \x1b[N;{mod}~
            SpecialKey::F5 => Ok(format!("\x1b[15;{}~", modifier_code).into_bytes()),
            SpecialKey::F6 => Ok(format!("\x1b[17;{}~", modifier_code).into_bytes()),
            SpecialKey::F7 => Ok(format!("\x1b[18;{}~", modifier_code).into_bytes()),
            SpecialKey::F8 => Ok(format!("\x1b[19;{}~", modifier_code).into_bytes()),
            SpecialKey::F9 => Ok(format!("\x1b[20;{}~", modifier_code).into_bytes()),
            SpecialKey::F10 => Ok(format!("\x1b[21;{}~", modifier_code).into_bytes()),
            SpecialKey::F11 => Ok(format!("\x1b[23;{}~", modifier_code).into_bytes()),
            SpecialKey::F12 => Ok(format!("\x1b[24;{}~", modifier_code).into_bytes()),

            // These don't support modifiers in standard xterm
            _ => Ok(key.base_sequence().to_vec()),
        }
    }

    /// Calculate the modifier code for xterm-style sequences.
    ///
    /// The modifier code is: 1 + (shift ? 1 : 0) + (alt ? 2 : 0) + (ctrl ? 4 : 0)
    fn modifier_code(&self) -> u8 {
        1 + (if self.shift { 1 } else { 0 })
            + (if self.alt { 2 } else { 0 })
            + (if self.ctrl { 4 } else { 0 })
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arrow_keys() {
        let input = KeyInput {
            key: Some(SpecialKey::Up),
            ..Default::default()
        };
        assert_eq!(input.encode().unwrap(), b"\x1b[A");

        let input = KeyInput {
            key: Some(SpecialKey::Down),
            ..Default::default()
        };
        assert_eq!(input.encode().unwrap(), b"\x1b[B");
    }

    #[test]
    fn test_ctrl_c() {
        let input = KeyInput {
            text: Some("c".into()),
            ctrl: true,
            ..Default::default()
        };
        assert_eq!(input.encode().unwrap(), vec![0x03]); // ETX
    }

    #[test]
    fn test_ctrl_d() {
        let input = KeyInput {
            text: Some("d".into()),
            ctrl: true,
            ..Default::default()
        };
        assert_eq!(input.encode().unwrap(), vec![0x04]); // EOT
    }

    #[test]
    fn test_ctrl_z() {
        let input = KeyInput {
            text: Some("z".into()),
            ctrl: true,
            ..Default::default()
        };
        assert_eq!(input.encode().unwrap(), vec![0x1a]); // SUB
    }

    #[test]
    fn test_shift_up() {
        let input = KeyInput {
            key: Some(SpecialKey::Up),
            shift: true,
            ..Default::default()
        };
        assert_eq!(input.encode().unwrap(), b"\x1b[1;2A");
    }

    #[test]
    fn test_ctrl_up() {
        let input = KeyInput {
            key: Some(SpecialKey::Up),
            ctrl: true,
            ..Default::default()
        };
        assert_eq!(input.encode().unwrap(), b"\x1b[1;5A");
    }

    #[test]
    fn test_alt_up() {
        let input = KeyInput {
            key: Some(SpecialKey::Up),
            alt: true,
            ..Default::default()
        };
        assert_eq!(input.encode().unwrap(), b"\x1b[1;3A");
    }

    #[test]
    fn test_function_keys() {
        let input = KeyInput {
            key: Some(SpecialKey::F1),
            ..Default::default()
        };
        assert_eq!(input.encode().unwrap(), b"\x1bOP");

        let input = KeyInput {
            key: Some(SpecialKey::F5),
            ..Default::default()
        };
        assert_eq!(input.encode().unwrap(), b"\x1b[15~");
    }

    #[test]
    fn test_text() {
        let input = KeyInput {
            text: Some("hello".into()),
            ..Default::default()
        };
        assert_eq!(input.encode().unwrap(), b"hello");
    }

    #[test]
    fn test_alt_text() {
        let input = KeyInput {
            text: Some("x".into()),
            alt: true,
            ..Default::default()
        };
        assert_eq!(input.encode().unwrap(), b"\x1bx");
    }
}
