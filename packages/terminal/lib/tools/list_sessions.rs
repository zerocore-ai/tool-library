//! terminal__list tool implementation.

use std::sync::Arc;

use rmcp::{ErrorData as McpError, Json};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::session::{SessionInfo, SessionManager};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Output for list_sessions tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListSessionsOutput {
    /// List of active sessions.
    pub sessions: Vec<SessionInfo>,

    /// Number of active sessions.
    pub count: usize,
}

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Handle the list_sessions tool call.
pub async fn handle_list_sessions(
    manager: Arc<SessionManager>,
) -> Result<Json<ListSessionsOutput>, McpError> {
    let sessions = manager.list().await;
    let count = sessions.len();

    Ok(Json(ListSessionsOutput { sessions, count }))
}
