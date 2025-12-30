//! Shared types and error definitions for the terminal MCP server.

use rmcp::ErrorData as McpError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

//--------------------------------------------------------------------------------------------------
// Types: Error
//--------------------------------------------------------------------------------------------------

/// Terminal server error types.
#[derive(Debug, thiserror::Error)]
pub enum TerminalError {
    #[error("PTY error: {0}")]
    Pty(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Maximum sessions reached ({0})")]
    MaxSessionsReached(usize),

    #[error("Session already destroyed: {0}")]
    SessionDestroyed(String),

    #[error("Session has error: {0}")]
    SessionError(String),

    #[error("No input provided (need text or key)")]
    NoInput,

    #[error("Invalid key: {0}")]
    InvalidKey(String),

    #[error("Invalid prompt pattern: {0}")]
    InvalidPattern(#[from] regex::Error),

    #[error("Process has exited with code {0:?}")]
    ProcessExited(Option<i32>),

    #[error("Program not found: {0}")]
    ProgramNotFound(String),

    #[error("Wait timeout after {0}ms")]
    WaitTimeout(u64),

    #[error("Channel closed")]
    ChannelClosed,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl TerminalError {
    /// Get the error code for this error variant.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Pty(_) => "PTY_ERROR",
            Self::Io(_) => "IO_ERROR",
            Self::SessionNotFound(_) => "SESSION_NOT_FOUND",
            Self::MaxSessionsReached(_) => "MAX_SESSIONS",
            Self::SessionDestroyed(_) => "SESSION_DESTROYED",
            Self::SessionError(_) => "SESSION_ERROR",
            Self::NoInput => "NO_INPUT",
            Self::InvalidKey(_) => "INVALID_KEY",
            Self::InvalidPattern(_) => "INVALID_PATTERN",
            Self::ProcessExited(_) => "PROCESS_EXITED",
            Self::ProgramNotFound(_) => "PROGRAM_NOT_FOUND",
            Self::WaitTimeout(_) => "WAIT_TIMEOUT",
            Self::ChannelClosed => "CHANNEL_CLOSED",
        }
    }

    /// Convert to MCP error with structured data.
    pub fn to_mcp_error(&self) -> McpError {
        McpError::invalid_params(self.to_string(), Some(json!({ "code": self.code() })))
    }
}

//--------------------------------------------------------------------------------------------------
// Types: Common
//--------------------------------------------------------------------------------------------------

/// Terminal dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Dimensions {
    pub rows: u16,
    pub cols: u16,
}

impl Default for Dimensions {
    fn default() -> Self {
        Self { rows: 24, cols: 80 }
    }
}

/// Cursor position in the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CursorPosition {
    pub row: u16,
    pub col: u16,
}

impl Default for CursorPosition {
    fn default() -> Self {
        Self { row: 0, col: 0 }
    }
}

/// Output format for terminal content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// Strip ANSI codes, return plain text.
    #[default]
    Plain,
    /// Preserve ANSI codes.
    Raw,
}

/// View mode for reading terminal content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ViewMode {
    /// Current visible buffer (rows x cols) - for TUI apps.
    Screen,
    /// All output since last read - for command output.
    #[default]
    New,
    /// Historical output with pagination - for review.
    Scrollback,
}

/// Result type for terminal operations.
pub type Result<T> = std::result::Result<T, TerminalError>;
