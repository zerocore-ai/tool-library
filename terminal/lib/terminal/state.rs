//! Per-session terminal state.

use vte::Parser;

use crate::config::GlobalConfig;
use crate::pty::PtySession;
use crate::types::{CursorPosition, Dimensions, OutputFormat, Result, ViewMode};

use super::emulator::ScreenPerformer;
use super::prompt::PromptDetector;
use super::screen::ScreenBuffer;
use super::scrollback::ScrollbackBuffer;
use super::tracker::OutputTracker;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Per-session terminal state that coordinates all emulation components.
pub struct TerminalState {
    /// The PTY session.
    pty: PtySession,

    /// Screen buffer (visible terminal).
    screen: ScreenBuffer,

    /// Scrollback buffer (historical output).
    scrollback: ScrollbackBuffer,

    /// Output tracker (for "new" view mode).
    tracker: OutputTracker,

    /// Prompt detector.
    prompt_detector: PromptDetector,

    /// VT parser.
    vt_parser: Parser,

    /// Terminal dimensions.
    rows: u16,
    cols: u16,

    /// Whether the process has exited.
    exited: bool,

    /// Exit code if exited.
    exit_code: Option<i32>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl TerminalState {
    /// Create a new terminal state.
    pub fn new(pty: PtySession, config: &GlobalConfig) -> Result<Self> {
        let size = pty.size();

        let prompt_detector = PromptDetector::new(&config.prompt_pattern)?;

        Ok(Self {
            pty,
            screen: ScreenBuffer::new(size.rows, size.cols),
            scrollback: ScrollbackBuffer::new(config.scrollback_limit),
            tracker: OutputTracker::new(),
            prompt_detector,
            vt_parser: Parser::new(),
            rows: size.rows,
            cols: size.cols,
            exited: false,
            exit_code: None,
        })
    }

    /// Process raw PTY output through VT parser.
    pub fn process_output(&mut self, data: &[u8]) {
        // Track raw output for "new" view
        self.tracker.append(data);

        // Process through VT parser
        for byte in data {
            let mut performer = ScreenPerformer::new(&mut self.screen, &mut self.scrollback);
            self.vt_parser.advance(&mut performer, *byte);
        }
    }

    /// Mark the session as exited.
    pub fn set_exited(&mut self, code: Option<i32>) {
        self.exited = true;
        self.exit_code = code;
    }

    /// Check if the process has exited.
    pub fn exited(&self) -> bool {
        self.exited
    }

    /// Get exit code if exited.
    pub fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }

    /// Get terminal dimensions.
    pub fn dimensions(&self) -> Dimensions {
        Dimensions {
            rows: self.rows,
            cols: self.cols,
        }
    }

    /// Get screen reference.
    pub fn screen(&self) -> &ScreenBuffer {
        &self.screen
    }

    /// Get PTY reference.
    pub fn pty(&self) -> &PtySession {
        &self.pty
    }

    /// Get mutable PTY reference.
    pub fn pty_mut(&mut self) -> &mut PtySession {
        &mut self.pty
    }

    /// Get a clone of the PTY writer for async operations.
    ///
    /// Use this to write to the PTY without holding the session lock
    /// across await points.
    pub fn writer(&self) -> std::sync::Arc<std::sync::Mutex<Box<dyn std::io::Write + Send>>> {
        self.pty.writer()
    }

    /// Get tracker reference.
    pub fn tracker(&self) -> &OutputTracker {
        &self.tracker
    }

    /// Get mutable tracker reference.
    pub fn tracker_mut(&mut self) -> &mut OutputTracker {
        &mut self.tracker
    }

    /// Get prompt detector reference.
    pub fn prompt_detector(&self) -> &PromptDetector {
        &self.prompt_detector
    }

    /// Get cursor position.
    pub fn cursor(&self) -> CursorPosition {
        self.screen.cursor()
    }

    /// Check if prompt is detected in current output.
    pub fn is_prompt_detected(&self) -> bool {
        let content = self.tracker.peek(OutputFormat::Plain);
        self.prompt_detector.detect(&content)
    }

    /// Read content based on view mode.
    pub fn read(&mut self, view: ViewMode, format: OutputFormat) -> String {
        match view {
            ViewMode::Screen => self.screen.render(format),
            ViewMode::New => self.tracker.take(format),
            ViewMode::Scrollback => self.scrollback.get_all(format),
        }
    }

    /// Read content with pagination (for scrollback).
    pub fn read_scrollback(&self, offset: usize, limit: usize, format: OutputFormat) -> String {
        self.scrollback.get(offset, limit, format)
    }

    /// Peek at new content without consuming.
    pub fn peek_new(&self, format: OutputFormat) -> String {
        self.tracker.peek(format)
    }

    /// Check if there's new content.
    pub fn has_new_content(&self) -> bool {
        self.tracker.has_content()
    }

    /// Clear the tracker (mark as read).
    pub fn mark_read(&mut self) {
        self.tracker.clear();
    }

    /// Get scrollback line count.
    pub fn scrollback_lines(&self) -> usize {
        self.scrollback.len()
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl std::fmt::Debug for TerminalState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerminalState")
            .field("dimensions", &self.dimensions())
            .field("exited", &self.exited)
            .field("exit_code", &self.exit_code)
            .field("cursor", &self.cursor())
            .finish()
    }
}
