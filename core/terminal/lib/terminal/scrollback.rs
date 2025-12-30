//! Scrollback buffer for historical terminal output.

use std::collections::VecDeque;

use crate::types::OutputFormat;

use super::screen::ScrollbackLine;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Ring buffer for lines that scroll off the top of the screen.
#[derive(Debug)]
pub struct ScrollbackBuffer {
    lines: VecDeque<ScrollbackLine>,
    max_lines: usize,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl ScrollbackBuffer {
    /// Create a new scrollback buffer with the given maximum size.
    pub fn new(max_lines: usize) -> Self {
        Self {
            lines: VecDeque::new(),
            max_lines,
        }
    }

    /// Push lines that scrolled off screen.
    pub fn push(&mut self, lines: Vec<ScrollbackLine>) {
        for line in lines {
            if self.lines.len() >= self.max_lines {
                self.lines.pop_front();
            }
            self.lines.push_back(line);
        }
    }

    /// Push a single line.
    pub fn push_line(&mut self, line: ScrollbackLine) {
        if self.lines.len() >= self.max_lines {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
    }

    /// Get lines with pagination.
    ///
    /// - `offset`: Lines from end (0 = most recent)
    /// - `limit`: Maximum lines to return
    pub fn get(&self, offset: usize, limit: usize, format: OutputFormat) -> String {
        let total = self.lines.len();
        if total == 0 || offset >= total {
            return String::new();
        }

        let start = total.saturating_sub(offset + limit);
        let end = total.saturating_sub(offset);

        self.lines
            .range(start..end)
            .map(|line| match format {
                OutputFormat::Plain => line.plain.as_str(),
                OutputFormat::Raw => line.raw.as_str(),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Get all lines.
    pub fn get_all(&self, format: OutputFormat) -> String {
        self.get(0, self.lines.len(), format)
    }

    /// Total lines stored.
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Check if buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.lines.clear();
    }

    /// Get maximum capacity.
    pub fn capacity(&self) -> usize {
        self.max_lines
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_and_get() {
        let mut buffer = ScrollbackBuffer::new(100);

        buffer.push_line(ScrollbackLine {
            plain: "line1".into(),
            raw: "line1".into(),
        });
        buffer.push_line(ScrollbackLine {
            plain: "line2".into(),
            raw: "line2".into(),
        });

        assert_eq!(buffer.len(), 2);
        let content = buffer.get_all(OutputFormat::Plain);
        assert!(content.contains("line1"));
        assert!(content.contains("line2"));
    }

    #[test]
    fn test_max_lines() {
        let mut buffer = ScrollbackBuffer::new(3);

        for i in 0..5 {
            buffer.push_line(ScrollbackLine {
                plain: format!("line{}", i),
                raw: format!("line{}", i),
            });
        }

        assert_eq!(buffer.len(), 3);
        let content = buffer.get_all(OutputFormat::Plain);
        assert!(!content.contains("line0"));
        assert!(!content.contains("line1"));
        assert!(content.contains("line2"));
        assert!(content.contains("line3"));
        assert!(content.contains("line4"));
    }

    #[test]
    fn test_pagination() {
        let mut buffer = ScrollbackBuffer::new(100);

        for i in 0..10 {
            buffer.push_line(ScrollbackLine {
                plain: format!("line{}", i),
                raw: format!("line{}", i),
            });
        }

        // Get last 3 lines
        let content = buffer.get(0, 3, OutputFormat::Plain);
        assert!(content.contains("line7"));
        assert!(content.contains("line8"));
        assert!(content.contains("line9"));
        assert!(!content.contains("line6"));

        // Get 3 lines with offset of 3
        let content = buffer.get(3, 3, OutputFormat::Plain);
        assert!(content.contains("line4"));
        assert!(content.contains("line5"));
        assert!(content.contains("line6"));
    }
}
