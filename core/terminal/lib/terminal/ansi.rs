//! ANSI escape code handling.

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// State machine for stripping ANSI codes.
#[derive(Debug, Default)]
enum StripState {
    #[default]
    Normal,
    Escape,
    Csi,
    Osc,
    OscEscape,
}

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Strip ANSI escape codes from a string.
pub fn strip_ansi(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut state = StripState::Normal;

    for c in input.chars() {
        match state {
            StripState::Normal => {
                if c == '\x1b' {
                    state = StripState::Escape;
                } else {
                    result.push(c);
                }
            }
            StripState::Escape => {
                match c {
                    '[' => state = StripState::Csi,
                    ']' => state = StripState::Osc,
                    '(' | ')' | '*' | '+' | '-' | '.' | '/' => {
                        // Character set designation - skip next char
                        state = StripState::Normal;
                    }
                    'D' | 'E' | 'H' | 'M' | 'N' | 'O' | 'P' | 'V' | 'W' | 'X' | 'Z' | '\\'
                    | '^' | '_' | '`' | 'c' | 'n' | 'o' | '|' | '}' | '~' | '=' | '>' | '7'
                    | '8' => {
                        // Single-character escape sequences
                        state = StripState::Normal;
                    }
                    _ => {
                        // Unknown escape, output escape and char
                        state = StripState::Normal;
                    }
                }
            }
            StripState::Csi => {
                // CSI sequence ends with letter (A-Z, a-z) or @, `, {, |, }, ~
                if c.is_ascii_alphabetic() || matches!(c, '@' | '`' | '{' | '|' | '}' | '~') {
                    state = StripState::Normal;
                }
                // Continue consuming CSI parameters
            }
            StripState::Osc => {
                // OSC sequence ends with BEL (\x07) or ST (\x1b\)
                if c == '\x07' {
                    state = StripState::Normal;
                } else if c == '\x1b' {
                    state = StripState::OscEscape;
                }
                // Continue consuming OSC content
            }
            StripState::OscEscape => {
                if c == '\\' {
                    state = StripState::Normal;
                } else {
                    // Not ST, back to OSC
                    state = StripState::Osc;
                }
            }
        }
    }

    result
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_ansi() {
        assert_eq!(strip_ansi("hello world"), "hello world");
    }

    #[test]
    fn test_strip_colors() {
        assert_eq!(strip_ansi("\x1b[31mred\x1b[0m"), "red");
        assert_eq!(strip_ansi("\x1b[1;32mgreen\x1b[0m"), "green");
    }

    #[test]
    fn test_strip_cursor() {
        assert_eq!(strip_ansi("\x1b[Hstart"), "start");
        assert_eq!(strip_ansi("\x1b[2Jclear"), "clear");
        assert_eq!(strip_ansi("\x1b[10;20Hpos"), "pos");
    }

    #[test]
    fn test_strip_osc_bel() {
        assert_eq!(strip_ansi("\x1b]0;title\x07content"), "content");
    }

    #[test]
    fn test_strip_osc_st() {
        assert_eq!(strip_ansi("\x1b]0;title\x1b\\content"), "content");
    }

    #[test]
    fn test_mixed() {
        let input = "\x1b[32mHello\x1b[0m \x1b[1mWorld\x1b[0m!";
        assert_eq!(strip_ansi(input), "Hello World!");
    }

    #[test]
    fn test_preserve_newlines() {
        let input = "line1\n\x1b[32mline2\x1b[0m\nline3";
        assert_eq!(strip_ansi(input), "line1\nline2\nline3");
    }
}
