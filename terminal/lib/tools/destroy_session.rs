//! terminal__destroy tool implementation.

use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ErrorData as McpError, Json};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::session::SessionManager;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Input for destroy_session tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DestroySessionInput {
    /// Session ID to destroy.
    pub session_id: String,

    /// Force kill (SIGKILL) instead of graceful termination (SIGTERM).
    #[serde(default)]
    pub force: bool,
}

/// Output for destroy_session tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DestroySessionOutput {
    /// Whether the session was successfully destroyed.
    pub destroyed: bool,

    /// Exit code if the process terminated gracefully.
    pub exit_code: Option<i32>,
}

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Handle the destroy_session tool call.
pub async fn handle_destroy_session(
    manager: Arc<SessionManager>,
    params: Parameters<DestroySessionInput>,
) -> Result<Json<DestroySessionOutput>, McpError> {
    let input = params.0;

    let result = manager
        .destroy_session(&input.session_id, input.force)
        .await
        .map_err(|e| e.to_mcp_error())?;

    Ok(Json(DestroySessionOutput {
        destroyed: result.destroyed,
        exit_code: result.exit_code,
    }))
}
