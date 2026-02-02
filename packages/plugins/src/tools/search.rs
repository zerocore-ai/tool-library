//! plugins__search tool implementation.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ErrorData as McpError, Json};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::config;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Plugin type filter for search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PluginType {
    Agent,
    Persona,
    Command,
    Tool,
    Snippet,
    Pack,
}

/// Input for the search tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchInput {
    /// Search query text (full-text search on name, description, keywords).
    pub query: String,

    /// Filter by plugin type: agent, persona, command, tool, snippet, pack.
    #[serde(default)]
    pub plugin_type: Option<PluginType>,

    /// Maximum number of results to return (1-100, default: 20).
    #[serde(default)]
    pub limit: Option<u32>,
}

/// A single search result item.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchResultItem {
    /// Plugin namespace (e.g., "appcypher").
    pub namespace: String,

    /// Plugin name (e.g., "genesis").
    pub name: String,

    /// Full plugin reference (namespace/name).
    pub reference: String,

    /// Short description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Plugin type.
    pub plugin_type: String,

    /// Latest version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_version: Option<String>,

    /// Total download count.
    pub total_downloads: i64,

    /// Star count.
    pub star_count: i64,
}

/// Output for the search tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchOutput {
    /// Search results.
    pub results: Vec<SearchResultItem>,

    /// Number of results returned.
    pub count: usize,
}

//--------------------------------------------------------------------------------------------------
// Types: API Response
//--------------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ApiSearchResponse {
    data: Vec<ApiSearchResultItem>,
}

#[derive(Debug, Deserialize)]
struct ApiSearchResultItem {
    artifact: ApiArtifactSummary,
}

#[derive(Debug, Deserialize)]
struct ApiArtifactSummary {
    namespace: String,
    name: String,
    description: Option<String>,
    artifact_type: String,
    #[serde(default)]
    total_downloads: i64,
    #[serde(default)]
    star_count: i64,
    latest_version: Option<String>,
}

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Handle the search tool call.
pub async fn handle_search(params: Parameters<SearchInput>) -> Result<Json<SearchOutput>, McpError> {
    let input = params.0;
    let cfg = config();

    let limit = input.limit.unwrap_or(20).clamp(1, 100);
    let mut url = format!(
        "{}/api/v1/search?q={}&page=1&per_page={}",
        cfg.registry_url,
        urlencoding::encode(&input.query),
        limit
    );

    if let Some(plugin_type) = input.plugin_type {
        let type_str = match plugin_type {
            PluginType::Agent => "agent",
            PluginType::Persona => "persona",
            PluginType::Command => "command",
            PluginType::Tool => "tool",
            PluginType::Snippet => "snippet",
            PluginType::Pack => "pack",
        };
        url.push_str(&format!("&artifact_type={}", type_str));
    }

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| McpError::internal_error(format!("Failed to search registry: {}", e), None))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(McpError::internal_error(
            format!("Registry search failed ({}): {}", status, body),
            None,
        ));
    }

    let api_response: ApiSearchResponse = response
        .json()
        .await
        .map_err(|e| McpError::internal_error(format!("Failed to parse search results: {}", e), None))?;

    let results: Vec<SearchResultItem> = api_response
        .data
        .into_iter()
        .map(|item| SearchResultItem {
            reference: format!("{}/{}", item.artifact.namespace, item.artifact.name),
            namespace: item.artifact.namespace,
            name: item.artifact.name,
            description: item.artifact.description,
            plugin_type: item.artifact.artifact_type,
            latest_version: item.artifact.latest_version,
            total_downloads: item.artifact.total_downloads,
            star_count: item.artifact.star_count,
        })
        .collect();

    let count = results.len();

    Ok(Json(SearchOutput { results, count }))
}
