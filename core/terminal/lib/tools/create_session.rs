//! terminal__create tool implementation.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ErrorData as McpError, Json};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::session::{is_shell_program, CreateSessionOptions, SessionManager};
use crate::types::Dimensions;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Input for create_session tool.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct CreateSessionInput {
    /// Program to run (default: $SHELL or /bin/bash).
    #[serde(default)]
    pub program: Option<String>,

    /// Program arguments.
    #[serde(default)]
    pub args: Vec<String>,

    /// Terminal height in rows (default: 24).
    #[serde(default)]
    pub rows: Option<u16>,

    /// Terminal width in columns (default: 80).
    #[serde(default)]
    pub cols: Option<u16>,

    /// Additional environment variables.
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Working directory.
    #[serde(default)]
    pub cwd: Option<String>,

    /// Wait for shell prompt before returning (default: true for shells).
    #[serde(default)]
    pub wait_ready: Option<bool>,

    /// Timeout for wait_ready in milliseconds (default: 5000).
    #[serde(default)]
    pub ready_timeout_ms: Option<u64>,
}

/// Output for create_session tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateSessionOutput {
    /// Unique session identifier.
    pub session_id: String,

    /// Process ID of the spawned program.
    pub pid: Option<u32>,

    /// Resolved program path.
    pub program: String,

    /// Terminal dimensions.
    pub dimensions: Dimensions,
}

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Handle the create_session tool call.
pub async fn handle_create_session(
    manager: Arc<SessionManager>,
    params: Parameters<CreateSessionInput>,
) -> Result<Json<CreateSessionOutput>, McpError> {
    let input = params.0;

    let opts = CreateSessionOptions {
        program: input.program.clone(),
        args: input.args,
        rows: input.rows,
        cols: input.cols,
        env: input.env,
        cwd: input.cwd.map(PathBuf::from),
        wait_ready: input.wait_ready,
        ready_timeout_ms: input.ready_timeout_ms,
    };

    // Create the session
    let info = manager
        .create_session(opts)
        .await
        .map_err(|e| e.to_mcp_error())?;

    // Determine if we should wait for ready
    let program = info.program.clone();
    let should_wait = input.wait_ready.unwrap_or_else(|| is_shell_program(&program));

    if should_wait {
        let timeout_ms = input.ready_timeout_ms.unwrap_or(5000);

        // Get the session and wait for prompt
        if let Ok(session) = manager.get(&info.session_id).await {
            let mut session = session.lock().await;

            // Drain initial output and wait for prompt
            let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);

            while std::time::Instant::now() < deadline {
                session.drain_reader().ok();

                if session.state.is_prompt_detected() {
                    break;
                }

                if session.state.exited() {
                    break;
                }

                tokio::time::sleep(Duration::from_millis(50)).await;
            }

            // Clear the tracker so "new" view starts fresh
            session.state.tracker_mut().clear();
        }
    }

    Ok(Json(CreateSessionOutput {
        session_id: info.session_id,
        pid: info.pid,
        program: info.program,
        dimensions: info.dimensions,
    }))
}
