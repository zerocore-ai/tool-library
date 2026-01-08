//! Cursor state management.

use crate::types::CursorPosition;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Cursor state within the terminal screen.
#[derive(Debug, Clone)]
pub struct CursorState {
    /// Current row position (0-indexed).
    pub row: u16,

    /// Current column position (0-indexed).
    pub col: u16,

    /// Whether the cursor is visible.
    pub visible: bool,

    /// Saved cursor position (for save/restore operations).
    saved: Option<(u16, u16)>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl CursorState {
    /// Create a new cursor at the origin.
    pub fn new() -> Self {
        Self {
            row: 0,
            col: 0,
            visible: true,
            saved: None,
        }
    }

    /// Get the cursor position.
    pub fn position(&self) -> CursorPosition {
        CursorPosition {
            row: self.row,
            col: self.col,
        }
    }

    /// Move cursor to absolute position.
    pub fn move_to(&mut self, row: u16, col: u16, max_rows: u16, max_cols: u16) {
        self.row = row.min(max_rows.saturating_sub(1));
        self.col = col.min(max_cols.saturating_sub(1));
    }

    /// Move cursor up by n rows.
    pub fn move_up(&mut self, n: u16) {
        self.row = self.row.saturating_sub(n);
    }

    /// Move cursor down by n rows.
    pub fn move_down(&mut self, n: u16, max_rows: u16) {
        self.row = (self.row + n).min(max_rows.saturating_sub(1));
    }

    /// Move cursor left by n columns.
    pub fn move_left(&mut self, n: u16) {
        self.col = self.col.saturating_sub(n);
    }

    /// Move cursor right by n columns.
    pub fn move_right(&mut self, n: u16, max_cols: u16) {
        self.col = (self.col + n).min(max_cols.saturating_sub(1));
    }

    /// Move cursor to beginning of line.
    pub fn carriage_return(&mut self) {
        self.col = 0;
    }

    /// Move cursor to beginning of next line.
    pub fn newline(&mut self, max_rows: u16) -> bool {
        self.col = 0;
        if self.row + 1 >= max_rows {
            // Need to scroll
            true
        } else {
            self.row += 1;
            false
        }
    }

    /// Move cursor down without carriage return.
    pub fn line_feed(&mut self, max_rows: u16) -> bool {
        if self.row + 1 >= max_rows {
            true
        } else {
            self.row += 1;
            false
        }
    }

    /// Advance cursor by one column, wrapping if necessary.
    pub fn advance(&mut self, max_cols: u16, max_rows: u16) -> bool {
        self.col += 1;
        if self.col >= max_cols {
            self.col = 0;
            self.line_feed(max_rows)
        } else {
            false
        }
    }

    /// Advance cursor by width columns (for wide characters).
    pub fn advance_by(&mut self, width: u16, max_cols: u16, max_rows: u16) -> bool {
        let mut needs_scroll = false;
        for _ in 0..width {
            if self.advance(max_cols, max_rows) {
                needs_scroll = true;
            }
        }
        needs_scroll
    }

    /// Save current cursor position.
    pub fn save(&mut self) {
        self.saved = Some((self.row, self.col));
    }

    /// Restore saved cursor position.
    pub fn restore(&mut self) {
        if let Some((row, col)) = self.saved {
            self.row = row;
            self.col = col;
        }
    }

    /// Move to column n (1-indexed in VT, we convert to 0-indexed).
    pub fn move_to_column(&mut self, col: u16, max_cols: u16) {
        self.col = col.saturating_sub(1).min(max_cols.saturating_sub(1));
    }

    /// Move to row n (1-indexed in VT, we convert to 0-indexed).
    pub fn move_to_row(&mut self, row: u16, max_rows: u16) {
        self.row = row.saturating_sub(1).min(max_rows.saturating_sub(1));
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl Default for CursorState {
    fn default() -> Self {
        Self::new()
    }
}
