//! Output tracker for the "new" view mode.

use crate::types::OutputFormat;

use super::ansi::strip_ansi;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Tracks output since last read for the "new" view mode.
#[derive(Debug, Default)]
pub struct OutputTracker {
    /// Raw bytes since last read.
    buffer: Vec<u8>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl OutputTracker {
    /// Create a new output tracker.
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    /// Append new PTY output.
    pub fn append(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    /// Get and clear tracked output.
    pub fn take(&mut self, format: OutputFormat) -> String {
        let content = self.format_content(format);
        self.buffer.clear();
        content
    }

    /// Peek at output without clearing.
    pub fn peek(&self, format: OutputFormat) -> String {
        self.format_content(format)
    }

    /// Check if there's new content.
    pub fn has_content(&self) -> bool {
        !self.buffer.is_empty()
    }

    /// Clear without returning.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Get raw buffer length.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Check if buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Format the content according to output format.
    fn format_content(&self, format: OutputFormat) -> String {
        let raw = String::from_utf8_lossy(&self.buffer).to_string();

        match format {
            OutputFormat::Plain => strip_ansi(&raw),
            OutputFormat::Raw => raw,
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
    fn test_append_and_take() {
        let mut tracker = OutputTracker::new();

        tracker.append(b"hello ");
        tracker.append(b"world");

        let content = tracker.take(OutputFormat::Raw);
        assert_eq!(content, "hello world");
        assert!(!tracker.has_content());
    }

    #[test]
    fn test_peek() {
        let mut tracker = OutputTracker::new();

        tracker.append(b"test");

        let content = tracker.peek(OutputFormat::Raw);
        assert_eq!(content, "test");
        assert!(tracker.has_content()); // Not cleared

        let content = tracker.take(OutputFormat::Raw);
        assert_eq!(content, "test");
        assert!(!tracker.has_content()); // Now cleared
    }

    #[test]
    fn test_clear() {
        let mut tracker = OutputTracker::new();

        tracker.append(b"data");
        assert!(tracker.has_content());

        tracker.clear();
        assert!(!tracker.has_content());
    }
}
