//! Screen buffer representing the visible terminal.

use unicode_width::UnicodeWidthChar;

use crate::types::{CursorPosition, Dimensions, OutputFormat};

use super::cursor::CursorState;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Cell attributes (colors, styles).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CellAttributes {
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub blink: bool,
    pub reverse: bool,
    pub hidden: bool,
    pub strikethrough: bool,
    pub foreground: Option<Color>,
    pub background: Option<Color>,
}

/// Terminal color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    /// Standard 16 colors (0-15).
    Indexed(u8),
    /// 24-bit RGB color.
    Rgb(u8, u8, u8),
}

/// A single cell in the terminal screen.
#[derive(Debug, Clone)]
pub struct Cell {
    /// The character in this cell.
    pub character: char,

    /// Display width: 0 = continuation of wide char, 1 = normal, 2 = wide.
    pub width: u8,

    /// Cell attributes.
    pub attrs: CellAttributes,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            character: ' ',
            width: 1,
            attrs: CellAttributes::default(),
        }
    }
}

/// A line that has scrolled off the screen.
#[derive(Debug, Clone)]
pub struct ScrollbackLine {
    /// Plain text content (no ANSI codes).
    pub plain: String,

    /// Raw content with ANSI codes preserved.
    pub raw: String,
}

/// The visible terminal screen buffer.
#[derive(Debug)]
pub struct ScreenBuffer {
    /// Grid of cells (rows x cols).
    cells: Vec<Vec<Cell>>,

    /// Cursor state.
    cursor: CursorState,

    /// Terminal dimensions.
    rows: u16,
    cols: u16,

    /// Current attributes for new characters.
    current_attrs: CellAttributes,

    /// Lines that have scrolled off and need to be pushed to scrollback.
    scrolled_lines: Vec<ScrollbackLine>,

    /// Whether alternate screen buffer is active.
    alternate_active: bool,

    /// Main screen buffer (saved when alternate is active).
    main_buffer: Option<Vec<Vec<Cell>>>,

    /// Main cursor (saved when alternate is active).
    main_cursor: Option<CursorState>,

    /// Window title from OSC sequences.
    title: Option<String>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl ScreenBuffer {
    /// Create a new screen buffer with the given dimensions.
    pub fn new(rows: u16, cols: u16) -> Self {
        let cells = vec![vec![Cell::default(); cols as usize]; rows as usize];

        Self {
            cells,
            cursor: CursorState::new(),
            rows,
            cols,
            current_attrs: CellAttributes::default(),
            scrolled_lines: Vec::new(),
            alternate_active: false,
            main_buffer: None,
            main_cursor: None,
            title: None,
        }
    }

    /// Get terminal dimensions.
    pub fn dimensions(&self) -> Dimensions {
        Dimensions {
            rows: self.rows,
            cols: self.cols,
        }
    }

    /// Get cursor position.
    pub fn cursor(&self) -> CursorPosition {
        self.cursor.position()
    }

    /// Get cursor visibility.
    pub fn cursor_visible(&self) -> bool {
        self.cursor.visible
    }

    /// Get mutable cursor access.
    pub fn cursor_mut(&mut self) -> &mut CursorState {
        &mut self.cursor
    }

    /// Get current attributes.
    pub fn current_attrs(&self) -> &CellAttributes {
        &self.current_attrs
    }

    /// Get mutable current attributes.
    pub fn current_attrs_mut(&mut self) -> &mut CellAttributes {
        &mut self.current_attrs
    }

    /// Reset current attributes to default.
    pub fn reset_attrs(&mut self) {
        self.current_attrs = CellAttributes::default();
    }

    /// Put a character at the current cursor position.
    pub fn put_char(&mut self, c: char) {
        let width = c.width().unwrap_or(1) as u8;

        // Handle wide characters at edge
        if width == 2 && self.cursor.col as usize + 1 >= self.cols as usize {
            // Wrap to next line
            let needs_scroll = self.cursor.newline(self.rows);
            if needs_scroll {
                self.scroll_up(1);
            }
        }

        let row = self.cursor.row as usize;
        let col = self.cursor.col as usize;

        if row < self.cells.len() && col < self.cells[row].len() {
            self.cells[row][col] = Cell {
                character: c,
                width,
                attrs: self.current_attrs.clone(),
            };

            // For wide chars, mark next cell as continuation
            if width == 2 && col + 1 < self.cells[row].len() {
                self.cells[row][col + 1] = Cell {
                    character: ' ',
                    width: 0,
                    attrs: self.current_attrs.clone(),
                };
            }
        }

        // Advance cursor
        let needs_scroll = self.cursor.advance_by(width as u16, self.cols, self.rows);
        if needs_scroll {
            self.scroll_up(1);
        }
    }

    /// Scroll the screen up by n lines.
    pub fn scroll_up(&mut self, n: u16) {
        let n = n as usize;
        if n == 0 || n >= self.cells.len() {
            return;
        }

        // Save scrolled lines (only if not in alternate buffer)
        if !self.alternate_active {
            for row in self.cells.drain(..n) {
                let plain = row.iter().map(|c| c.character).collect::<String>();
                // For now, raw is same as plain (ANSI rendering comes later)
                let raw = plain.clone();
                self.scrolled_lines.push(ScrollbackLine { plain, raw });
            }
        } else {
            self.cells.drain(..n);
        }

        // Add empty lines at bottom
        for _ in 0..n {
            self.cells.push(vec![Cell::default(); self.cols as usize]);
        }

        // Adjust cursor if needed
        if self.cursor.row >= n as u16 {
            self.cursor.row -= n as u16;
        } else {
            self.cursor.row = 0;
        }
    }

    /// Scroll the screen down by n lines.
    pub fn scroll_down(&mut self, n: u16) {
        let n = n as usize;
        if n == 0 || n >= self.cells.len() {
            return;
        }

        // Remove lines from bottom
        self.cells.truncate(self.cells.len() - n);

        // Add empty lines at top
        for _ in 0..n {
            self.cells.insert(0, vec![Cell::default(); self.cols as usize]);
        }

        // Adjust cursor
        self.cursor.row = (self.cursor.row + n as u16).min(self.rows - 1);
    }

    /// Erase from cursor to end of screen.
    pub fn erase_below(&mut self) {
        self.erase_line_right();
        let row = self.cursor.row as usize + 1;
        for r in row..self.cells.len() {
            self.clear_row(r);
        }
    }

    /// Erase from start of screen to cursor.
    pub fn erase_above(&mut self) {
        self.erase_line_left();
        let row = self.cursor.row as usize;
        for r in 0..row {
            self.clear_row(r);
        }
    }

    /// Erase entire screen.
    pub fn erase_all(&mut self) {
        for r in 0..self.cells.len() {
            self.clear_row(r);
        }
    }

    /// Erase from cursor to end of line.
    pub fn erase_line_right(&mut self) {
        let row = self.cursor.row as usize;
        let col = self.cursor.col as usize;
        if row < self.cells.len() {
            for c in col..self.cells[row].len() {
                self.cells[row][c] = Cell::default();
            }
        }
    }

    /// Erase from start of line to cursor.
    pub fn erase_line_left(&mut self) {
        let row = self.cursor.row as usize;
        let col = self.cursor.col as usize;
        if row < self.cells.len() {
            for c in 0..=col.min(self.cells[row].len().saturating_sub(1)) {
                self.cells[row][c] = Cell::default();
            }
        }
    }

    /// Erase entire line.
    pub fn erase_line(&mut self) {
        let row = self.cursor.row as usize;
        self.clear_row(row);
    }

    /// Clear a row.
    fn clear_row(&mut self, row: usize) {
        if row < self.cells.len() {
            for cell in &mut self.cells[row] {
                *cell = Cell::default();
            }
        }
    }

    /// Take lines that scrolled off (for scrollback buffer).
    pub fn take_scrolled_lines(&mut self) -> Vec<ScrollbackLine> {
        std::mem::take(&mut self.scrolled_lines)
    }

    /// Render screen content as string.
    pub fn render(&self, format: OutputFormat) -> String {
        // First, collect all lines
        let mut lines: Vec<String> = Vec::new();

        for row in &self.cells {
            // Trim trailing spaces
            let line: String = row
                .iter()
                .filter(|c| c.width > 0) // Skip continuation cells
                .map(|c| c.character)
                .collect::<String>()
                .trim_end()
                .to_string();

            lines.push(line);
        }

        // Trim trailing empty lines
        while lines.last().is_some_and(|l| l.is_empty()) {
            lines.pop();
        }

        let result = lines.join("\n");

        match format {
            OutputFormat::Plain => result,
            OutputFormat::Raw => result, // TODO: Add ANSI codes
        }
    }

    /// Set window title.
    pub fn set_title(&mut self, title: String) {
        self.title = Some(title);
    }

    /// Get window title.
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Switch to alternate screen buffer.
    pub fn enter_alternate_buffer(&mut self) {
        if self.alternate_active {
            return;
        }

        self.alternate_active = true;
        self.main_buffer = Some(std::mem::replace(
            &mut self.cells,
            vec![vec![Cell::default(); self.cols as usize]; self.rows as usize],
        ));
        self.main_cursor = Some(std::mem::replace(&mut self.cursor, CursorState::new()));
    }

    /// Switch back to main screen buffer.
    pub fn exit_alternate_buffer(&mut self) {
        if !self.alternate_active {
            return;
        }

        self.alternate_active = false;
        if let Some(buffer) = self.main_buffer.take() {
            self.cells = buffer;
        }
        if let Some(cursor) = self.main_cursor.take() {
            self.cursor = cursor;
        }
    }

    /// Check if alternate buffer is active.
    pub fn is_alternate_active(&self) -> bool {
        self.alternate_active
    }

    /// Handle tab character.
    pub fn tab(&mut self) {
        // Move to next tab stop (every 8 columns)
        let next_tab = ((self.cursor.col / 8) + 1) * 8;
        self.cursor.col = next_tab.min(self.cols - 1);
    }

    /// Handle backspace character.
    pub fn backspace(&mut self) {
        self.cursor.move_left(1);
    }

    /// Handle carriage return.
    pub fn carriage_return(&mut self) {
        self.cursor.carriage_return();
    }

    /// Handle line feed.
    pub fn line_feed(&mut self) {
        let needs_scroll = self.cursor.line_feed(self.rows);
        if needs_scroll {
            self.scroll_up(1);
        }
    }

    /// Handle newline (CR + LF).
    pub fn newline(&mut self) {
        self.carriage_return();
        self.line_feed();
    }

    /// Insert n blank lines at cursor position.
    pub fn insert_lines(&mut self, n: u16) {
        let n = n as usize;
        let row = self.cursor.row as usize;

        if row >= self.cells.len() {
            return;
        }

        // Remove lines from bottom
        let remove_count = n.min(self.cells.len() - row);
        self.cells.truncate(self.cells.len() - remove_count);

        // Insert blank lines at cursor
        for _ in 0..remove_count {
            self.cells.insert(row, vec![Cell::default(); self.cols as usize]);
        }
    }

    /// Delete n lines at cursor position.
    pub fn delete_lines(&mut self, n: u16) {
        let n = n as usize;
        let row = self.cursor.row as usize;

        if row >= self.cells.len() {
            return;
        }

        let remove_count = n.min(self.cells.len() - row);

        // Remove lines at cursor
        for _ in 0..remove_count {
            if row < self.cells.len() {
                self.cells.remove(row);
            }
        }

        // Add blank lines at bottom
        for _ in 0..remove_count {
            self.cells.push(vec![Cell::default(); self.cols as usize]);
        }
    }

    /// Insert n blank characters at cursor position.
    pub fn insert_chars(&mut self, n: u16) {
        let row = self.cursor.row as usize;
        let col = self.cursor.col as usize;

        if row >= self.cells.len() {
            return;
        }

        let n = n as usize;
        let row_cells = &mut self.cells[row];

        // Shift characters right
        for _ in 0..n {
            if col < row_cells.len() {
                row_cells.pop();
                row_cells.insert(col, Cell::default());
            }
        }
    }

    /// Delete n characters at cursor position.
    pub fn delete_chars(&mut self, n: u16) {
        let row = self.cursor.row as usize;
        let col = self.cursor.col as usize;

        if row >= self.cells.len() {
            return;
        }

        let n = n as usize;
        let row_cells = &mut self.cells[row];

        // Remove characters and add spaces at end
        for _ in 0..n {
            if col < row_cells.len() {
                row_cells.remove(col);
                row_cells.push(Cell::default());
            }
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_screen() {
        let screen = ScreenBuffer::new(24, 80);
        assert_eq!(screen.rows, 24);
        assert_eq!(screen.cols, 80);
        assert_eq!(screen.cursor().row, 0);
        assert_eq!(screen.cursor().col, 0);
    }

    #[test]
    fn test_put_char() {
        let mut screen = ScreenBuffer::new(24, 80);
        screen.put_char('H');
        screen.put_char('i');

        let content = screen.render(OutputFormat::Plain);
        assert!(content.starts_with("Hi"));
    }

    #[test]
    fn test_wide_char() {
        let mut screen = ScreenBuffer::new(24, 80);
        screen.put_char('你');
        screen.put_char('好');

        let content = screen.render(OutputFormat::Plain);
        assert!(content.starts_with("你好"));
    }

    #[test]
    fn test_newline() {
        let mut screen = ScreenBuffer::new(24, 80);
        screen.put_char('A');
        screen.newline();
        screen.put_char('B');

        let content = screen.render(OutputFormat::Plain);
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "A");
        assert_eq!(lines[1], "B");
    }

    #[test]
    fn test_scroll() {
        let mut screen = ScreenBuffer::new(3, 10);
        for i in 0..5 {
            screen.put_char((b'A' + i) as char);
            screen.newline();
        }

        // Should have scrolled, keeping last 3 lines
        let scrolled = screen.take_scrolled_lines();
        assert!(!scrolled.is_empty());
    }

    #[test]
    fn test_erase_line() {
        let mut screen = ScreenBuffer::new(24, 80);
        screen.put_char('A');
        screen.put_char('B');
        screen.put_char('C');
        screen.cursor_mut().col = 1;
        screen.erase_line_right();

        let content = screen.render(OutputFormat::Plain);
        assert_eq!(content.trim(), "A");
    }
}
