//! Configuration for the terminal MCP server.

use serde::{Deserialize, Serialize};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Global configuration for the terminal server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// Default number of rows for new sessions.
    pub default_rows: u16,

    /// Default number of columns for new sessions.
    pub default_cols: u16,

    /// Default shell for new sessions.
    pub default_shell: String,

    /// Terminal type for TERM environment variable.
    pub term: String,

    /// Maximum lines to keep in scrollback per session.
    pub scrollback_limit: usize,

    /// Regex pattern to detect shell prompt.
    pub prompt_pattern: String,

    /// Maximum number of concurrent sessions.
    pub max_sessions: usize,
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            default_rows: 24,
            default_cols: 80,
            default_shell: std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into()),
            term: "xterm-256color".into(),
            scrollback_limit: 10000,
            prompt_pattern: r"\$\s*$|#\s*$|>\s*$".into(),
            max_sessions: 10,
        }
    }
}
