//! MCP server implementation.

use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{
    handler::server::tool::ToolRouter, model::ServerCapabilities, model::ServerInfo,
    model::{Implementation, ProtocolVersion}, tool, tool_handler, tool_router, ErrorData as McpError, Json,
    ServerHandler,
};

use crate::config::GlobalConfig;
use crate::session::SessionManager;
use crate::tools::{
    handle_create_session, handle_destroy_session, handle_get_info, handle_list_sessions,
    handle_read, handle_send, CreateSessionInput, CreateSessionOutput, DestroySessionInput,
    DestroySessionOutput, GetInfoInput, GetInfoOutput, ListSessionsOutput, ReadInput, ReadOutput,
    SendInput, SendOutput,
};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Terminal MCP server.
#[derive(Clone)]
pub struct Server {
    tool_router: ToolRouter<Self>,
    manager: Arc<SessionManager>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl Server {
    /// Create a new server with default configuration.
    pub fn new() -> Self {
        Self::with_config(GlobalConfig::default())
    }

    /// Create a new server with custom configuration.
    pub fn with_config(config: GlobalConfig) -> Self {
        let manager = Arc::new(SessionManager::new(config));
        Self {
            tool_router: Self::tool_router(),
            manager,
        }
    }

    /// Get the session manager.
    pub fn manager(&self) -> &Arc<SessionManager> {
        &self.manager
    }

    /// Shutdown the server, terminating all sessions.
    pub async fn shutdown(&self) {
        self.manager.shutdown().await;
    }
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations: Tool Router
//--------------------------------------------------------------------------------------------------

#[tool_router]
impl Server {
    /// Create a new terminal session.
    #[tool(
        name = "terminal__create",
        description = "Create a new terminal session running any program (shell by default). Returns a session_id for subsequent operations."
    )]
    async fn create_session(
        &self,
        params: Parameters<CreateSessionInput>,
    ) -> Result<Json<CreateSessionOutput>, McpError> {
        handle_create_session(self.manager.clone(), params).await
    }

    /// Destroy a terminal session.
    #[tool(
        name = "terminal__destroy",
        description = "Terminate a terminal session and clean up resources."
    )]
    async fn destroy_session(
        &self,
        params: Parameters<DestroySessionInput>,
    ) -> Result<Json<DestroySessionOutput>, McpError> {
        handle_destroy_session(self.manager.clone(), params).await
    }

    /// List all terminal sessions.
    #[tool(
        name = "terminal__list",
        description = "List all active terminal sessions."
    )]
    async fn list_sessions(&self) -> Result<Json<ListSessionsOutput>, McpError> {
        handle_list_sessions(self.manager.clone()).await
    }

    /// Send input to a terminal session.
    #[tool(
        name = "terminal__send",
        description = "Send input (text or special keys) to a terminal session. Optionally read output after sending."
    )]
    async fn send(&self, params: Parameters<SendInput>) -> Result<Json<SendOutput>, McpError> {
        handle_send(self.manager.clone(), params).await
    }

    /// Read output from a terminal session.
    #[tool(
        name = "terminal__read",
        description = "Read output from a terminal session. Supports screen view (TUI), new output (commands), and scrollback (history)."
    )]
    async fn read(&self, params: Parameters<ReadInput>) -> Result<Json<ReadOutput>, McpError> {
        handle_read(self.manager.clone(), params).await
    }

    /// Get information about a terminal session.
    #[tool(
        name = "terminal__info",
        description = "Get information about a terminal session without reading content."
    )]
    async fn get_info(
        &self,
        params: Parameters<GetInfoInput>,
    ) -> Result<Json<GetInfoOutput>, McpError> {
        handle_get_info(self.manager.clone(), params).await
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations: Server Handler
//--------------------------------------------------------------------------------------------------

#[tool_handler]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "Terminal MCP server providing PTY-based terminal sessions. \
                 Create sessions with terminal__create, send input with terminal__send, \
                 read output with terminal__read, and manage sessions with terminal__list \
                 and terminal__destroy."
                    .to_string(),
            ),
        }
    }
}
