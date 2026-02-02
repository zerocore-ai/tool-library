//! terminal__read tool implementation.

use std::sync::Arc;
use std::time::{Duration, Instant};

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ErrorData as McpError, Json};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::session::SessionManager;
use crate::types::{CursorPosition, Dimensions, OutputFormat, ViewMode};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Input for read tool.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ReadInput {
    /// Session ID to read from.
    pub session_id: String,

    /// View mode: "screen", "new", or "scrollback".
    #[serde(default)]
    pub view: Option<String>,

    /// Output format: "plain" or "raw".
    #[serde(default)]
    pub format: Option<String>,

    /// Maximum wait time in milliseconds (0 = immediate).
    #[serde(default)]
    pub timeout_ms: Option<u64>,

    /// Wait until no output for N milliseconds.
    #[serde(default)]
    pub wait_idle_ms: Option<u64>,

    /// Wait for shell prompt.
    #[serde(default)]
    pub wait_for_prompt: Option<bool>,

    /// Pagination offset for scrollback (0 = most recent).
    #[serde(default)]
    pub offset: Option<usize>,

    /// Pagination limit for scrollback.
    #[serde(default)]
    pub limit: Option<usize>,
}

/// Output for read tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadOutput {
    /// Terminal content.
    pub content: String,

    /// Number of lines in content.
    pub lines: usize,

    /// Cursor position (screen view only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<CursorPosition>,

    /// Terminal dimensions.
    pub dimensions: Dimensions,

    /// Whether there is new content since last read.
    pub has_new_content: bool,

    /// Whether a shell prompt was detected.
    pub prompt_detected: bool,

    /// Whether output was idle for wait_idle_ms.
    pub idle: bool,

    /// Whether the process has exited.
    pub exited: bool,

    /// Exit code if exited.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
}

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Parse view mode from string.
fn parse_view_mode(s: Option<&str>) -> ViewMode {
    match s {
        Some("screen") => ViewMode::Screen,
        Some("scrollback") => ViewMode::Scrollback,
        _ => ViewMode::New,
    }
}

/// Parse output format from string.
fn parse_output_format(s: Option<&str>) -> OutputFormat {
    match s {
        Some("raw") => OutputFormat::Raw,
        _ => OutputFormat::Plain,
    }
}

/// Handle the read tool call (internal, returns ReadOutput directly).
pub async fn handle_read_internal(
    manager: Arc<SessionManager>,
    input: ReadInput,
) -> Result<ReadOutput, McpError> {
    let view = parse_view_mode(input.view.as_deref());
    let format = parse_output_format(input.format.as_deref());
    let timeout_ms = input.timeout_ms.unwrap_or(0);
    let wait_idle_ms = input.wait_idle_ms.unwrap_or(0);
    let wait_for_prompt = input.wait_for_prompt.unwrap_or(false);
    let offset = input.offset.unwrap_or(0);
    let limit = input.limit.unwrap_or(1000);

    // Get the session
    let session = manager
        .get(&input.session_id)
        .await
        .map_err(|e| e.to_mcp_error())?;

    let mut session = session.lock().await;

    // Wait conditions
    let deadline = Instant::now() + Duration::from_millis(timeout_ms.max(1));
    let mut last_output = Instant::now();
    let mut is_idle = false;

    loop {
        // Drain reader
        let had_data = session.drain_reader().unwrap_or(false);
        if had_data {
            last_output = Instant::now();
        }

        // Check exit
        if session.state.exited() {
            break;
        }

        // Check prompt
        if wait_for_prompt && session.state.is_prompt_detected() {
            break;
        }

        // Check idle
        if wait_idle_ms > 0 && last_output.elapsed() >= Duration::from_millis(wait_idle_ms) {
            is_idle = true;
            break;
        }

        // Check timeout
        if Instant::now() >= deadline {
            break;
        }

        // Don't busy wait if we have wait conditions
        if timeout_ms > 0 || wait_idle_ms > 0 || wait_for_prompt {
            tokio::time::sleep(Duration::from_millis(10)).await;
        } else {
            break;
        }
    }

    // Final drain
    session.drain_reader().ok();

    // Get content based on view mode
    let content = match view {
        ViewMode::Screen => session.state.screen().render(format),
        ViewMode::New => session.state.read(ViewMode::New, format),
        ViewMode::Scrollback => session.state.read_scrollback(offset, limit, format),
    };

    let lines = content.lines().count();

    // Cursor only for screen view
    let cursor = if view == ViewMode::Screen {
        Some(session.state.cursor())
    } else {
        None
    };

    let dimensions = session.state.dimensions();
    let has_new_content = session.state.has_new_content();
    let prompt_detected = session.state.is_prompt_detected();
    let exited = session.state.exited();
    let exit_code = session.state.exit_code();

    Ok(ReadOutput {
        content,
        lines,
        cursor,
        dimensions,
        has_new_content,
        prompt_detected,
        idle: is_idle,
        exited,
        exit_code,
    })
}

/// Handle the read tool call.
pub async fn handle_read(
    manager: Arc<SessionManager>,
    params: Parameters<ReadInput>,
) -> Result<Json<ReadOutput>, McpError> {
    let output = handle_read_internal(manager, params.0).await?;
    Ok(Json(output))
}
