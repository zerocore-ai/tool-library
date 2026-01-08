//! plugins__resolve tool implementation.

use radical_core::resolver::{FilePluginResolver, RegistryClient};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ErrorData as McpError, Json};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::config;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Plugin type for resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResolvePluginType {
    Agent,
    Persona,
    Command,
    Tool,
    Snippet,
}

/// Input for the resolve tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ResolveInput {
    /// Plugin reference in format [namespace/]name[@version].
    /// Examples: "genesis", "radical/genesis", "radical/genesis@1.0.0", "commit@1"
    pub reference: String,

    /// Type of plugin to resolve: agent, persona, command, tool, snippet.
    pub plugin_type: ResolvePluginType,
}

/// Output for the resolve tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ResolveOutput {
    /// Whether the plugin was successfully resolved.
    pub found: bool,

    /// Resolved namespace (null for unnamespaced local plugins).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,

    /// Plugin name.
    pub name: String,

    /// Resolved version (semver string, null if unversioned).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Where the plugin was resolved from.
    pub source: ResolveSource,

    /// Filesystem path to the resolved plugin (only for local source).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    /// Parsed manifest content (for tools: MCPB manifest.json; for spec-based: parsed metadata).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest: Option<serde_json::Value>,

    /// Raw content body (for spec-based plugins, excludes frontmatter).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// Source of the resolved plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResolveSource {
    Local,
    Registry,
}

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Handle the resolve tool call.
pub async fn handle_resolve(params: Parameters<ResolveInput>) -> Result<Json<ResolveOutput>, McpError> {
    let input = params.0;
    let cfg = config();

    // Build the resolver
    let mut resolver = FilePluginResolver::default();

    // Enable registry fallback if configured
    if cfg.use_registry_fallback {
        let client = RegistryClient::new().with_url(&cfg.registry_url);
        resolver = resolver.with_auto_install(client);
    }

    // Resolve based on plugin type
    match input.plugin_type {
        ResolvePluginType::Agent => resolve_agent(&resolver, &input.reference).await,
        ResolvePluginType::Persona => resolve_persona(&resolver, &input.reference).await,
        ResolvePluginType::Command => resolve_command(&resolver, &input.reference).await,
        ResolvePluginType::Snippet => resolve_snippet(&resolver, &input.reference).await,
        ResolvePluginType::Tool => resolve_tool(&resolver, &input.reference).await,
    }
}

/// Resolve an agent plugin.
async fn resolve_agent(
    resolver: &FilePluginResolver,
    reference: &str,
) -> Result<Json<ResolveOutput>, McpError> {
    match resolver.resolve_agent(reference).await {
        Ok(Some(resolved)) => {
            let namespace = resolved.plugin_ref.namespace().map(|s| s.to_string());
            let name = resolved.plugin_ref.name().to_string();
            let version = resolved.plugin_ref.version().map(|v| v.to_string());
            let path = resolved.path.to_string_lossy().to_string();

            // Determine source based on whether auto_install was triggered
            // For simplicity, we check if the path contains the namespace
            let source = if resolver.has_auto_install() && namespace.is_some() {
                ResolveSource::Registry
            } else {
                ResolveSource::Local
            };

            // Extract metadata and content
            let manifest = serde_json::to_value(&resolved.template.metadata).ok();
            let content = Some(resolved.template.body().to_string());

            Ok(Json(ResolveOutput {
                found: true,
                namespace,
                name,
                version,
                source,
                path: Some(path),
                manifest,
                content,
            }))
        }
        Ok(None) => Ok(Json(not_found_output(reference))),
        Err(e) => Err(McpError::internal_error(format!("Resolution failed: {}", e), None)),
    }
}

/// Resolve a persona plugin.
async fn resolve_persona(
    resolver: &FilePluginResolver,
    reference: &str,
) -> Result<Json<ResolveOutput>, McpError> {
    match resolver.resolve_persona(reference).await {
        Ok(Some(resolved)) => {
            let namespace = resolved.plugin_ref.namespace().map(|s| s.to_string());
            let name = resolved.plugin_ref.name().to_string();
            let version = resolved.plugin_ref.version().map(|v| v.to_string());
            let path = resolved.path.to_string_lossy().to_string();

            let source = if resolver.has_auto_install() && namespace.is_some() {
                ResolveSource::Registry
            } else {
                ResolveSource::Local
            };

            let manifest = serde_json::to_value(&resolved.template.metadata).ok();
            let content = Some(resolved.template.preamble().to_string());

            Ok(Json(ResolveOutput {
                found: true,
                namespace,
                name,
                version,
                source,
                path: Some(path),
                manifest,
                content,
            }))
        }
        Ok(None) => Ok(Json(not_found_output(reference))),
        Err(e) => Err(McpError::internal_error(format!("Resolution failed: {}", e), None)),
    }
}

/// Resolve a command plugin.
async fn resolve_command(
    resolver: &FilePluginResolver,
    reference: &str,
) -> Result<Json<ResolveOutput>, McpError> {
    match resolver.resolve_command(reference).await {
        Ok(Some(resolved)) => {
            let namespace = resolved.plugin_ref.namespace().map(|s| s.to_string());
            let name = resolved.plugin_ref.name().to_string();
            let version = resolved.plugin_ref.version().map(|v| v.to_string());
            let path = resolved.path.to_string_lossy().to_string();

            let source = if resolver.has_auto_install() && namespace.is_some() {
                ResolveSource::Registry
            } else {
                ResolveSource::Local
            };

            let manifest = serde_json::to_value(&resolved.template.metadata).ok();
            let content = Some(resolved.template.body().to_string());

            Ok(Json(ResolveOutput {
                found: true,
                namespace,
                name,
                version,
                source,
                path: Some(path),
                manifest,
                content,
            }))
        }
        Ok(None) => Ok(Json(not_found_output(reference))),
        Err(e) => Err(McpError::internal_error(format!("Resolution failed: {}", e), None)),
    }
}

/// Resolve a snippet plugin.
async fn resolve_snippet(
    resolver: &FilePluginResolver,
    reference: &str,
) -> Result<Json<ResolveOutput>, McpError> {
    match resolver.resolve_snippet(reference).await {
        Ok(Some(resolved)) => {
            let namespace = resolved.plugin_ref.namespace().map(|s| s.to_string());
            let name = resolved.plugin_ref.name().to_string();
            let version = resolved.plugin_ref.version().map(|v| v.to_string());
            let path = resolved.path.to_string_lossy().to_string();

            let source = if resolver.has_auto_install() && namespace.is_some() {
                ResolveSource::Registry
            } else {
                ResolveSource::Local
            };

            let manifest = serde_json::to_value(&resolved.template.metadata).ok();
            let content = Some(resolved.template.body().to_string());

            Ok(Json(ResolveOutput {
                found: true,
                namespace,
                name,
                version,
                source,
                path: Some(path),
                manifest,
                content,
            }))
        }
        Ok(None) => Ok(Json(not_found_output(reference))),
        Err(e) => Err(McpError::internal_error(format!("Resolution failed: {}", e), None)),
    }
}

/// Resolve a tool plugin.
async fn resolve_tool(
    resolver: &FilePluginResolver,
    reference: &str,
) -> Result<Json<ResolveOutput>, McpError> {
    match resolver.resolve_tool(reference).await {
        Ok(Some(resolved)) => {
            let namespace = resolved.plugin_ref.namespace().map(|s| s.to_string());
            let name = resolved.plugin_ref.name().to_string();
            let version = resolved.plugin_ref.version().map(|v| v.to_string());
            let path = resolved.path.to_string_lossy().to_string();

            let source = if resolver.has_auto_install() && namespace.is_some() {
                ResolveSource::Registry
            } else {
                ResolveSource::Local
            };

            // For tools, the manifest is the full MCPB manifest
            let manifest = serde_json::to_value(&resolved.template).ok();

            Ok(Json(ResolveOutput {
                found: true,
                namespace,
                name,
                version,
                source,
                path: Some(path),
                manifest,
                content: None, // Tools don't have content, just manifest
            }))
        }
        Ok(None) => Ok(Json(not_found_output(reference))),
        Err(e) => Err(McpError::internal_error(format!("Resolution failed: {}", e), None)),
    }
}

/// Create a not-found output.
fn not_found_output(reference: &str) -> ResolveOutput {
    // Parse the reference to extract name
    let name = reference
        .split('/')
        .last()
        .unwrap_or(reference)
        .split('@')
        .next()
        .unwrap_or(reference)
        .to_string();

    ResolveOutput {
        found: false,
        namespace: None,
        name,
        version: None,
        source: ResolveSource::Local,
        path: None,
        manifest: None,
        content: None,
    }
}
