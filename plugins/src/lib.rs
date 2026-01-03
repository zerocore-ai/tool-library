//! Plugins MCP server for searching and resolving plugins.

mod config;
mod tools;

use rmcp::{
    ErrorData as McpError, Json, ServerHandler,
    handler::server::tool::ToolRouter,
    handler::server::wrapper::Parameters,
    model::{ServerCapabilities, ServerInfo, Implementation, ProtocolVersion},
    tool, tool_router, tool_handler,
};

use crate::tools::{
    SearchInput, SearchOutput, handle_search,
    ResolveInput, ResolveOutput, handle_resolve,
};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

#[derive(Clone)]
pub struct Server {
    tool_router: ToolRouter<Self>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

#[tool_router]
impl Server {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Search for plugins on the registry by query and optional type filter")]
    async fn search(&self, params: Parameters<SearchInput>) -> Result<Json<SearchOutput>, McpError> {
        handle_search(params).await
    }

    #[tool(description = "Resolve a plugin reference to its manifest and content")]
    async fn resolve(&self, params: Parameters<ResolveInput>) -> Result<Json<ResolveOutput>, McpError> {
        handle_resolve(params).await
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

#[tool_handler]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: None,
        }
    }
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}
