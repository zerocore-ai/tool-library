//! terminal__info tool implementation.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ErrorData as McpError, Json};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::session::SessionManager;
use crate::types::{CursorPosition, Dimensions};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Input for get_info tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetInfoInput {
    /// Session ID to get info for.
    pub session_id: String,
}

/// Output for get_info tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetInfoOutput {
    /// Session identifier.
    pub session_id: String,

    /// Running program.
    pub program: String,

    /// Program arguments.
    pub args: Vec<String>,

    /// Process ID.
    pub pid: Option<u32>,

    /// Creation timestamp.
    pub created_at: DateTime<Utc>,

    /// Cursor position.
    pub cursor: CursorPosition,

    /// Terminal dimensions.
    pub dimensions: Dimensions,

    /// Whether the process has exited.
    pub exited: bool,

    /// Exit code if exited.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,

    /// Whether the session is healthy.
    pub healthy: bool,

    /// Current working directory (if detectable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Try to detect the current working directory of a process.
#[cfg(target_os = "linux")]
fn detect_cwd(pid: u32) -> Option<String> {
    std::fs::read_link(format!("/proc/{}/cwd", pid))
        .ok()
        .and_then(|p| p.to_str().map(String::from))
}

#[cfg(target_os = "macos")]
fn detect_cwd(pid: u32) -> Option<String> {
    use std::process::Command;

    // Use lsof to get cwd on macOS
    let output = Command::new("lsof")
        .args(["-a", "-p", &pid.to_string(), "-d", "cwd", "-Fn"])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(path) = line.strip_prefix('n') {
            return Some(path.to_string());
        }
    }

    None
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn detect_cwd(_pid: u32) -> Option<String> {
    None
}

/// Handle the get_info tool call.
pub async fn handle_get_info(
    manager: Arc<SessionManager>,
    params: Parameters<GetInfoInput>,
) -> Result<Json<GetInfoOutput>, McpError> {
    let input = params.0;

    // Get the session
    let session = manager
        .get(&input.session_id)
        .await
        .map_err(|e| e.to_mcp_error())?;

    let session = session.lock().await;

    // Try to detect CWD
    let cwd = session.state.pty().pid().and_then(detect_cwd);

    Ok(Json(GetInfoOutput {
        session_id: session.id.clone(),
        program: session.program.clone(),
        args: session.args.clone(),
        pid: session.state.pty().pid(),
        created_at: session.created_at_utc,
        cursor: session.state.cursor(),
        dimensions: session.state.dimensions(),
        exited: session.state.exited(),
        exit_code: session.state.exit_code(),
        healthy: session.is_healthy(),
        cwd,
    }))
}
