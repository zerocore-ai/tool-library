//! VT100/ANSI terminal emulator using vte crate.

use vte::{Params, Perform};

use super::screen::{Color, ScreenBuffer};
use super::scrollback::ScrollbackBuffer;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// VT sequence performer that updates screen state.
pub struct ScreenPerformer<'a> {
    screen: &'a mut ScreenBuffer,
    scrollback: &'a mut ScrollbackBuffer,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl<'a> ScreenPerformer<'a> {
    /// Create a new performer.
    pub fn new(screen: &'a mut ScreenBuffer, scrollback: &'a mut ScrollbackBuffer) -> Self {
        Self { screen, scrollback }
    }

    /// Flush scrolled lines to scrollback buffer.
    pub fn flush_scrollback(&mut self) {
        let lines = self.screen.take_scrolled_lines();
        self.scrollback.push(lines);
    }

    /// Get parameter value with default.
    fn param(params: &Params, idx: usize, default: u16) -> u16 {
        params
            .iter()
            .nth(idx)
            .and_then(|p| p.first().copied())
            .filter(|&v| v != 0)
            .unwrap_or(default)
    }

    /// Handle SGR (Select Graphic Rendition) parameters.
    fn handle_sgr(&mut self, params: &Params) {
        // If no params, reset
        if params.is_empty() {
            self.screen.reset_attrs();
            return;
        }

        // Collect params into a vec so we can iterate without borrowing issues
        let params_vec: Vec<Vec<u16>> = params.iter().map(|p| p.to_vec()).collect();
        let mut iter = params_vec.iter();

        while let Some(param) = iter.next() {
            let code = param.first().copied().unwrap_or(0);

            match code {
                0 => self.screen.reset_attrs(),
                1 => self.screen.current_attrs_mut().bold = true,
                2 => self.screen.current_attrs_mut().dim = true,
                3 => self.screen.current_attrs_mut().italic = true,
                4 => self.screen.current_attrs_mut().underline = true,
                5 | 6 => self.screen.current_attrs_mut().blink = true,
                7 => self.screen.current_attrs_mut().reverse = true,
                8 => self.screen.current_attrs_mut().hidden = true,
                9 => self.screen.current_attrs_mut().strikethrough = true,
                21 => self.screen.current_attrs_mut().bold = false, // Some terminals use this
                22 => {
                    let attrs = self.screen.current_attrs_mut();
                    attrs.bold = false;
                    attrs.dim = false;
                }
                23 => self.screen.current_attrs_mut().italic = false,
                24 => self.screen.current_attrs_mut().underline = false,
                25 => self.screen.current_attrs_mut().blink = false,
                27 => self.screen.current_attrs_mut().reverse = false,
                28 => self.screen.current_attrs_mut().hidden = false,
                29 => self.screen.current_attrs_mut().strikethrough = false,
                30..=37 => {
                    self.screen.current_attrs_mut().foreground =
                        Some(Color::Indexed((code - 30) as u8))
                }
                38 => {
                    // Extended foreground color
                    if let Some(next) = iter.next() {
                        let subcode = next.first().copied().unwrap_or(0);
                        match subcode {
                            5 => {
                                // 256-color mode
                                if let Some(color) = iter.next() {
                                    let idx = color.first().copied().unwrap_or(0) as u8;
                                    self.screen.current_attrs_mut().foreground =
                                        Some(Color::Indexed(idx));
                                }
                            }
                            2 => {
                                // 24-bit RGB
                                let r =
                                    iter.next().and_then(|p| p.first().copied()).unwrap_or(0) as u8;
                                let g =
                                    iter.next().and_then(|p| p.first().copied()).unwrap_or(0) as u8;
                                let b =
                                    iter.next().and_then(|p| p.first().copied()).unwrap_or(0) as u8;
                                self.screen.current_attrs_mut().foreground =
                                    Some(Color::Rgb(r, g, b));
                            }
                            _ => {}
                        }
                    }
                }
                39 => self.screen.current_attrs_mut().foreground = None,
                40..=47 => {
                    self.screen.current_attrs_mut().background =
                        Some(Color::Indexed((code - 40) as u8))
                }
                48 => {
                    // Extended background color
                    if let Some(next) = iter.next() {
                        let subcode = next.first().copied().unwrap_or(0);
                        match subcode {
                            5 => {
                                if let Some(color) = iter.next() {
                                    let idx = color.first().copied().unwrap_or(0) as u8;
                                    self.screen.current_attrs_mut().background =
                                        Some(Color::Indexed(idx));
                                }
                            }
                            2 => {
                                let r =
                                    iter.next().and_then(|p| p.first().copied()).unwrap_or(0) as u8;
                                let g =
                                    iter.next().and_then(|p| p.first().copied()).unwrap_or(0) as u8;
                                let b =
                                    iter.next().and_then(|p| p.first().copied()).unwrap_or(0) as u8;
                                self.screen.current_attrs_mut().background =
                                    Some(Color::Rgb(r, g, b));
                            }
                            _ => {}
                        }
                    }
                }
                49 => self.screen.current_attrs_mut().background = None,
                90..=97 => {
                    self.screen.current_attrs_mut().foreground =
                        Some(Color::Indexed((code - 90 + 8) as u8))
                }
                100..=107 => {
                    self.screen.current_attrs_mut().background =
                        Some(Color::Indexed((code - 100 + 8) as u8))
                }
                _ => {}
            }
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl Perform for ScreenPerformer<'_> {
    fn print(&mut self, c: char) {
        self.screen.put_char(c);
        self.flush_scrollback();
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            0x07 => {} // BEL - ignore
            0x08 => self.screen.backspace(),        // BS
            0x09 => self.screen.tab(),               // HT
            0x0A => {                                // LF
                self.screen.line_feed();
                self.flush_scrollback();
            }
            0x0B => {                                // VT (same as LF)
                self.screen.line_feed();
                self.flush_scrollback();
            }
            0x0C => {                                // FF (same as LF)
                self.screen.line_feed();
                self.flush_scrollback();
            }
            0x0D => self.screen.carriage_return(),   // CR
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, action: char) {
        let dims = self.screen.dimensions();

        // Check for private mode indicator
        let is_private = intermediates.first() == Some(&b'?');

        match (action, is_private) {
            // Cursor movement
            ('A', false) => {
                // CUU - Cursor Up
                let n = Self::param(params, 0, 1);
                self.screen.cursor_mut().move_up(n);
            }
            ('B', false) => {
                // CUD - Cursor Down
                let n = Self::param(params, 0, 1);
                self.screen.cursor_mut().move_down(n, dims.rows);
            }
            ('C', false) => {
                // CUF - Cursor Forward
                let n = Self::param(params, 0, 1);
                self.screen.cursor_mut().move_right(n, dims.cols);
            }
            ('D', false) => {
                // CUB - Cursor Back
                let n = Self::param(params, 0, 1);
                self.screen.cursor_mut().move_left(n);
            }
            ('E', false) => {
                // CNL - Cursor Next Line
                let n = Self::param(params, 0, 1);
                self.screen.cursor_mut().move_down(n, dims.rows);
                self.screen.cursor_mut().carriage_return();
            }
            ('F', false) => {
                // CPL - Cursor Previous Line
                let n = Self::param(params, 0, 1);
                self.screen.cursor_mut().move_up(n);
                self.screen.cursor_mut().carriage_return();
            }
            ('G', false) => {
                // CHA - Cursor Horizontal Absolute
                let col = Self::param(params, 0, 1);
                self.screen.cursor_mut().move_to_column(col, dims.cols);
            }
            ('H', false) | ('f', false) => {
                // CUP/HVP - Cursor Position
                let row = Self::param(params, 0, 1);
                let col = Self::param(params, 1, 1);
                self.screen.cursor_mut().move_to(
                    row.saturating_sub(1),
                    col.saturating_sub(1),
                    dims.rows,
                    dims.cols,
                );
            }
            ('d', false) => {
                // VPA - Vertical Position Absolute
                let row = Self::param(params, 0, 1);
                self.screen.cursor_mut().move_to_row(row, dims.rows);
            }

            // Erase operations
            ('J', false) => {
                // ED - Erase Display
                let mode = Self::param(params, 0, 0);
                match mode {
                    0 => self.screen.erase_below(),
                    1 => self.screen.erase_above(),
                    2 | 3 => self.screen.erase_all(),
                    _ => {}
                }
            }
            ('K', false) => {
                // EL - Erase Line
                let mode = Self::param(params, 0, 0);
                match mode {
                    0 => self.screen.erase_line_right(),
                    1 => self.screen.erase_line_left(),
                    2 => self.screen.erase_line(),
                    _ => {}
                }
            }

            // Insert/Delete
            ('L', false) => {
                // IL - Insert Lines
                let n = Self::param(params, 0, 1);
                self.screen.insert_lines(n);
            }
            ('M', false) => {
                // DL - Delete Lines
                let n = Self::param(params, 0, 1);
                self.screen.delete_lines(n);
            }
            ('@', false) => {
                // ICH - Insert Characters
                let n = Self::param(params, 0, 1);
                self.screen.insert_chars(n);
            }
            ('P', false) => {
                // DCH - Delete Characters
                let n = Self::param(params, 0, 1);
                self.screen.delete_chars(n);
            }

            // Scroll
            ('S', false) => {
                // SU - Scroll Up
                let n = Self::param(params, 0, 1);
                self.screen.scroll_up(n);
                self.flush_scrollback();
            }
            ('T', false) => {
                // SD - Scroll Down
                let n = Self::param(params, 0, 1);
                self.screen.scroll_down(n);
            }

            // SGR - Select Graphic Rendition
            ('m', false) => {
                self.handle_sgr(params);
            }

            // Private modes (DECSET/DECRST)
            ('h', true) | ('l', true) => {
                let enable = action == 'h';
                for param in params.iter() {
                    if let Some(&mode) = param.first() {
                        match mode {
                            25 => {
                                // DECTCEM - Cursor visibility
                                self.screen.cursor_mut().visible = enable;
                            }
                            47 | 1047 => {
                                // Alternate screen buffer (without save/restore cursor)
                                if enable {
                                    self.screen.enter_alternate_buffer();
                                } else {
                                    self.screen.exit_alternate_buffer();
                                }
                            }
                            1049 => {
                                // Alternate screen buffer with save/restore cursor
                                if enable {
                                    self.screen.cursor_mut().save();
                                    self.screen.enter_alternate_buffer();
                                } else {
                                    self.screen.exit_alternate_buffer();
                                    self.screen.cursor_mut().restore();
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }

            // Cursor save/restore (ANSI.SYS style)
            ('s', false) => self.screen.cursor_mut().save(),
            ('u', false) => self.screen.cursor_mut().restore(),

            _ => {}
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], _ignore: bool, byte: u8) {
        match (intermediates, byte) {
            // Save cursor (DECSC)
            ([], b'7') => self.screen.cursor_mut().save(),
            // Restore cursor (DECRC)
            ([], b'8') => self.screen.cursor_mut().restore(),
            // Index (IND) - move down, scroll if at bottom
            ([], b'D') => {
                self.screen.line_feed();
                self.flush_scrollback();
            }
            // Next line (NEL) - CR + LF
            ([], b'E') => {
                self.screen.newline();
                self.flush_scrollback();
            }
            // Reverse index (RI) - move up, scroll down if at top
            ([], b'M') => {
                let cursor = self.screen.cursor_mut();
                if cursor.row == 0 {
                    self.screen.scroll_down(1);
                } else {
                    cursor.move_up(1);
                }
            }
            // Reset (RIS)
            ([], b'c') => {
                let dims = self.screen.dimensions();
                *self.screen = ScreenBuffer::new(dims.rows, dims.cols);
            }
            _ => {}
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if params.is_empty() {
            return;
        }

        // Parse OSC command
        let cmd = std::str::from_utf8(params[0]).ok();

        match cmd {
            Some("0") | Some("2") => {
                // Set window title
                if params.len() > 1 {
                    if let Ok(title) = std::str::from_utf8(params[1]) {
                        self.screen.set_title(title.to_string());
                    }
                }
            }
            _ => {
                // Ignore other OSC sequences
            }
        }
    }

    // DCS sequences - ignore
    fn hook(&mut self, _params: &Params, _intermediates: &[u8], _ignore: bool, _action: char) {}
    fn unhook(&mut self) {}
    fn put(&mut self, _byte: u8) {}
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::OutputFormat;

    fn process_input(input: &[u8]) -> (ScreenBuffer, ScrollbackBuffer) {
        let mut screen = ScreenBuffer::new(24, 80);
        let mut scrollback = ScrollbackBuffer::new(1000);
        let mut parser = vte::Parser::new();

        for byte in input {
            let mut performer = ScreenPerformer::new(&mut screen, &mut scrollback);
            parser.advance(&mut performer, *byte);
        }

        (screen, scrollback)
    }

    #[test]
    fn test_simple_text() {
        let (screen, _) = process_input(b"Hello, World!");
        let content = screen.render(OutputFormat::Plain);
        assert!(content.contains("Hello, World!"));
    }

    #[test]
    fn test_cursor_movement() {
        let (screen, _) = process_input(b"ABC\x1b[2D*");
        let content = screen.render(OutputFormat::Plain);
        assert!(content.contains("A*C"));
    }

    #[test]
    fn test_newlines() {
        let (screen, _) = process_input(b"Line1\r\nLine2");
        let content = screen.render(OutputFormat::Plain);
        let lines: Vec<&str> = content.lines().collect();
        assert!(lines.len() >= 2);
        assert_eq!(lines[0], "Line1");
        assert_eq!(lines[1], "Line2");
    }

    #[test]
    fn test_erase_display() {
        let (screen, _) = process_input(b"Hello\x1b[2J");
        let content = screen.render(OutputFormat::Plain);
        assert!(content.trim().is_empty());
    }

    #[test]
    fn test_color_codes() {
        let (screen, _) = process_input(b"\x1b[31mRed\x1b[0m Normal");
        let content = screen.render(OutputFormat::Plain);
        assert!(content.contains("Red Normal"));
    }
}
