use std::env;
use std::time::Duration;

use reqwest::redirect::Policy;
use rmcp::{
    ErrorData as McpError, Json, ServerHandler,
    handler::server::tool::ToolRouter,
    handler::server::wrapper::Parameters,
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::json;
use url::Url;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// Default request timeout in milliseconds.
const DEFAULT_TIMEOUT_MS: u64 = 30_000;

/// Default maximum content length in bytes (1 MB).
const DEFAULT_MAX_LENGTH: usize = 1_024 * 1_024;

/// Maximum allowed content length in bytes (10 MB).
const MAX_ALLOWED_LENGTH: usize = 10 * 1_024 * 1_024;

/// Maximum number of redirects to follow.
const MAX_REDIRECTS: usize = 10;

/// Default maximum number of search results.
const DEFAULT_MAX_RESULTS: usize = 10;

/// Maximum allowed search results.
const MAX_ALLOWED_RESULTS: usize = 50;

/// Minimum query length for search.
const MIN_QUERY_LENGTH: usize = 2;

/// User-Agent header for requests.
const USER_AGENT: &str = "Mozilla/5.0 (compatible; MCPWebServer/1.0)";

//--------------------------------------------------------------------------------------------------
// Types: Error
//--------------------------------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum WebError {
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Request failed: {0}")]
    RequestFailed(String),

    #[error("Request timeout after {0}ms")]
    Timeout(u64),

    #[error("Content too large: {size} bytes exceeds {max} bytes")]
    ContentTooLarge { size: usize, max: usize },

    #[error("Query too short: minimum {0} characters required")]
    QueryTooShort(usize),

    #[error("Too many redirects (max {0})")]
    TooManyRedirects(usize),

    #[error("Unsupported content type: {0}")]
    UnsupportedContentType(String),

    #[error("HTTP error: {0}")]
    HttpError(u16),

    #[error("Search provider error: {0}")]
    SearchProviderError(String),
}

impl WebError {
    /// Get the error code for this error variant.
    pub fn code(&self) -> &'static str {
        match self {
            WebError::InvalidUrl(_) => "INVALID_URL",
            WebError::RequestFailed(_) => "REQUEST_FAILED",
            WebError::Timeout(_) => "TIMEOUT",
            WebError::ContentTooLarge { .. } => "CONTENT_TOO_LARGE",
            WebError::QueryTooShort(_) => "QUERY_TOO_SHORT",
            WebError::TooManyRedirects(_) => "TOO_MANY_REDIRECTS",
            WebError::UnsupportedContentType(_) => "UNSUPPORTED_CONTENT_TYPE",
            WebError::HttpError(_) => "HTTP_ERROR",
            WebError::SearchProviderError(_) => "SEARCH_PROVIDER_ERROR",
        }
    }

    /// Convert to MCP error with structured data.
    pub fn to_mcp_error(&self) -> McpError {
        McpError::invalid_params(self.to_string(), Some(json!({ "code": self.code() })))
    }
}

/// Helper to convert errors to MCP error format.
fn to_mcp_error<E: Into<WebError>>(e: E) -> McpError {
    let err: WebError = e.into();
    err.to_mcp_error()
}

//--------------------------------------------------------------------------------------------------
// Types: Search Provider
//--------------------------------------------------------------------------------------------------

/// Available search providers in priority order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchProvider {
    /// Brave Search API (recommended, 2000 free/month)
    Brave,
    /// Tavily API (AI-optimized, 1000 free/month)
    Tavily,
    /// SerpAPI (Google results, 100 free/month)
    SerpApi,
    /// DuckDuckGo HTML scraping (no API key, unreliable)
    DuckDuckGo,
}

impl SearchProvider {
    /// Detect the best available provider from environment variables.
    pub fn detect() -> Self {
        if env::var("BRAVE_SEARCH_API_KEY").is_ok_and(|k| !k.is_empty()) {
            SearchProvider::Brave
        } else if env::var("TAVILY_API_KEY").is_ok_and(|k| !k.is_empty()) {
            SearchProvider::Tavily
        } else if env::var("SERPAPI_API_KEY").is_ok_and(|k| !k.is_empty()) {
            SearchProvider::SerpApi
        } else {
            SearchProvider::DuckDuckGo
        }
    }

    /// Get the display name of this provider.
    pub fn name(&self) -> &'static str {
        match self {
            SearchProvider::Brave => "Brave Search",
            SearchProvider::Tavily => "Tavily",
            SearchProvider::SerpApi => "SerpAPI",
            SearchProvider::DuckDuckGo => "DuckDuckGo",
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Types: web_fetch
//--------------------------------------------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct WebFetchInput {
    /// The URL to fetch (must be valid, HTTP auto-upgrades to HTTPS).
    pub url: String,

    /// Request timeout in milliseconds. Defaults to 30000 (30s).
    #[serde(default)]
    pub timeout_ms: Option<u64>,

    /// Maximum content length in bytes. Defaults to 1MB.
    #[serde(default)]
    pub max_length: Option<usize>,
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct WebFetchOutput {
    /// The fetched content converted to markdown.
    pub content: String,

    /// The final URL after redirects (if any).
    pub final_url: String,

    /// HTTP status code.
    pub status: u16,

    /// Content MIME type (e.g., "text/html", "application/json").
    pub content_type: String,

    /// Whether the content was truncated due to max_length.
    pub truncated: bool,
}

//--------------------------------------------------------------------------------------------------
// Types: web_search
//--------------------------------------------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct WebSearchInput {
    /// Search query (minimum 2 characters).
    pub query: String,

    /// Maximum number of results to return. Defaults to 10.
    #[serde(default)]
    pub max_results: Option<usize>,

    /// Only include results from these domains.
    #[serde(default)]
    pub allowed_domains: Option<Vec<String>>,

    /// Exclude results from these domains.
    #[serde(default)]
    pub blocked_domains: Option<Vec<String>>,
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchResult {
    /// Result title.
    pub title: String,

    /// Result URL.
    pub url: String,

    /// Result snippet/description.
    pub snippet: String,
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct WebSearchOutput {
    /// List of search results.
    pub results: Vec<SearchResult>,

    /// Number of results returned.
    pub count: usize,

    /// The search provider used.
    pub provider: String,
}

//--------------------------------------------------------------------------------------------------
// Types: Provider Response Schemas
//--------------------------------------------------------------------------------------------------

/// Brave Search API response.
#[derive(Debug, Deserialize)]
struct BraveSearchResponse {
    web: Option<BraveWebResults>,
}

#[derive(Debug, Deserialize)]
struct BraveWebResults {
    results: Vec<BraveResult>,
}

#[derive(Debug, Deserialize)]
struct BraveResult {
    title: String,
    url: String,
    description: Option<String>,
}

/// Tavily API response.
#[derive(Debug, Deserialize)]
struct TavilyResponse {
    results: Vec<TavilyResult>,
}

#[derive(Debug, Deserialize)]
struct TavilyResult {
    title: String,
    url: String,
    content: Option<String>,
}

/// SerpAPI response.
#[derive(Debug, Deserialize)]
struct SerpApiResponse {
    organic_results: Option<Vec<SerpApiResult>>,
}

#[derive(Debug, Deserialize)]
struct SerpApiResult {
    title: String,
    link: String,
    snippet: Option<String>,
}

//--------------------------------------------------------------------------------------------------
// Types: Server
//--------------------------------------------------------------------------------------------------

#[derive(Clone)]
pub struct Server {
    tool_router: ToolRouter<Self>,
    client: reqwest::Client,
    search_provider: SearchProvider,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl Server {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .redirect(Policy::limited(MAX_REDIRECTS))
            .build()
            .expect("Failed to build HTTP client");

        let search_provider = SearchProvider::detect();
        tracing::info!("Using search provider: {}", search_provider.name());

        Self {
            tool_router: Self::tool_router(),
            client,
            search_provider,
        }
    }

    /// Get the current search provider.
    pub fn search_provider(&self) -> SearchProvider {
        self.search_provider
    }

    /// Public wrapper for web_fetch (for testing).
    pub async fn fetch(&self, input: WebFetchInput) -> Result<WebFetchOutput, McpError> {
        self.web_fetch(Parameters(input)).await.map(|j| j.0)
    }

    /// Public wrapper for web_search (for testing).
    pub async fn search(&self, input: WebSearchInput) -> Result<WebSearchOutput, McpError> {
        self.web_search(Parameters(input)).await.map(|j| j.0)
    }
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

//--------------------------------------------------------------------------------------------------
// Functions: Helpers
//--------------------------------------------------------------------------------------------------

/// Validate and normalize a URL (HTTP → HTTPS upgrade).
fn validate_url(url_str: &str) -> Result<Url, WebError> {
    let mut url = Url::parse(url_str).map_err(|e| WebError::InvalidUrl(e.to_string()))?;

    // Upgrade HTTP to HTTPS
    if url.scheme() == "http" {
        url.set_scheme("https")
            .map_err(|_| WebError::InvalidUrl("Failed to upgrade to HTTPS".to_string()))?;
    }

    // Validate scheme
    if url.scheme() != "https" {
        return Err(WebError::InvalidUrl(format!(
            "Unsupported scheme: {}",
            url.scheme()
        )));
    }

    Ok(url)
}

/// Convert HTML content to markdown.
fn html_to_markdown(html: &str) -> String {
    htmd::convert(html).unwrap_or_else(|_| html.to_string())
}

/// Check if a URL's domain matches any in the given list.
fn domain_matches(url: &str, domains: &[String]) -> bool {
    if let Ok(parsed) = Url::parse(url) {
        if let Some(host) = parsed.host_str() {
            return domains
                .iter()
                .any(|d| host == d.as_str() || host.ends_with(&format!(".{}", d)));
        }
    }
    false
}

/// Apply domain filters to search results.
fn filter_results(
    mut results: Vec<SearchResult>,
    allowed_domains: &Option<Vec<String>>,
    blocked_domains: &Option<Vec<String>>,
    max_results: usize,
) -> Vec<SearchResult> {
    if let Some(allowed) = allowed_domains {
        results.retain(|r| domain_matches(&r.url, allowed));
    }
    if let Some(blocked) = blocked_domains {
        results.retain(|r| !domain_matches(&r.url, blocked));
    }
    results.truncate(max_results);
    results
}

//--------------------------------------------------------------------------------------------------
// Functions: Search Providers
//--------------------------------------------------------------------------------------------------

/// Search using Brave Search API.
async fn search_brave(
    client: &reqwest::Client,
    query: &str,
    max_results: usize,
) -> Result<Vec<SearchResult>, WebError> {
    let api_key = env::var("BRAVE_SEARCH_API_KEY")
        .map_err(|_| WebError::SearchProviderError("BRAVE_SEARCH_API_KEY not set".into()))?;

    let response = client
        .get("https://api.search.brave.com/res/v1/web/search")
        .header("X-Subscription-Token", api_key)
        .query(&[("q", query), ("count", &max_results.to_string())])
        .timeout(Duration::from_millis(DEFAULT_TIMEOUT_MS))
        .send()
        .await
        .map_err(|e| WebError::RequestFailed(e.to_string()))?;

    if !response.status().is_success() {
        return Err(WebError::HttpError(response.status().as_u16()));
    }

    let data: BraveSearchResponse = response
        .json()
        .await
        .map_err(|e| WebError::SearchProviderError(e.to_string()))?;

    let results = data
        .web
        .map(|w| {
            w.results
                .into_iter()
                .map(|r| SearchResult {
                    title: r.title,
                    url: r.url,
                    snippet: r.description.unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(results)
}

/// Search using Tavily API.
async fn search_tavily(
    client: &reqwest::Client,
    query: &str,
    max_results: usize,
) -> Result<Vec<SearchResult>, WebError> {
    let api_key = env::var("TAVILY_API_KEY")
        .map_err(|_| WebError::SearchProviderError("TAVILY_API_KEY not set".into()))?;

    let response = client
        .post("https://api.tavily.com/search")
        .json(&serde_json::json!({
            "api_key": api_key,
            "query": query,
            "max_results": max_results,
            "include_answer": false
        }))
        .timeout(Duration::from_millis(DEFAULT_TIMEOUT_MS))
        .send()
        .await
        .map_err(|e| WebError::RequestFailed(e.to_string()))?;

    if !response.status().is_success() {
        return Err(WebError::HttpError(response.status().as_u16()));
    }

    let data: TavilyResponse = response
        .json()
        .await
        .map_err(|e| WebError::SearchProviderError(e.to_string()))?;

    let results = data
        .results
        .into_iter()
        .map(|r| SearchResult {
            title: r.title,
            url: r.url,
            snippet: r.content.unwrap_or_default(),
        })
        .collect();

    Ok(results)
}

/// Search using SerpAPI.
async fn search_serpapi(
    client: &reqwest::Client,
    query: &str,
    max_results: usize,
) -> Result<Vec<SearchResult>, WebError> {
    let api_key = env::var("SERPAPI_API_KEY")
        .map_err(|_| WebError::SearchProviderError("SERPAPI_API_KEY not set".into()))?;

    let response = client
        .get("https://serpapi.com/search")
        .query(&[
            ("engine", "google"),
            ("q", query),
            ("api_key", &api_key),
            ("num", &max_results.to_string()),
        ])
        .timeout(Duration::from_millis(DEFAULT_TIMEOUT_MS))
        .send()
        .await
        .map_err(|e| WebError::RequestFailed(e.to_string()))?;

    if !response.status().is_success() {
        return Err(WebError::HttpError(response.status().as_u16()));
    }

    let data: SerpApiResponse = response
        .json()
        .await
        .map_err(|e| WebError::SearchProviderError(e.to_string()))?;

    let results = data
        .organic_results
        .map(|r| {
            r.into_iter()
                .map(|r| SearchResult {
                    title: r.title,
                    url: r.link,
                    snippet: r.snippet.unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(results)
}

/// Search using DuckDuckGo HTML scraping (fallback, unreliable).
async fn search_duckduckgo(
    client: &reqwest::Client,
    query: &str,
) -> Result<Vec<SearchResult>, WebError> {
    let search_url = format!(
        "https://html.duckduckgo.com/html/?q={}",
        urlencoding::encode(query)
    );

    let response = client
        .get(&search_url)
        .timeout(Duration::from_millis(DEFAULT_TIMEOUT_MS))
        .send()
        .await
        .map_err(|e| WebError::RequestFailed(e.to_string()))?;

    if !response.status().is_success() {
        return Err(WebError::HttpError(response.status().as_u16()));
    }

    let html = response
        .text()
        .await
        .map_err(|e| WebError::RequestFailed(e.to_string()))?;

    // Check for bot detection
    if html.contains("Unfortunately, bots use DuckDuckGo too") {
        return Err(WebError::SearchProviderError(
            "DuckDuckGo bot detection triggered. Consider using an API-based provider.".into(),
        ));
    }

    Ok(parse_duckduckgo_results(&html))
}

/// Parse DuckDuckGo HTML search results.
fn parse_duckduckgo_results(html: &str) -> Vec<SearchResult> {
    let document = Html::parse_document(html);
    let mut results = Vec::new();

    let result_selector = Selector::parse(".result").unwrap();
    let title_selector = Selector::parse(".result__title a").unwrap();
    let snippet_selector = Selector::parse(".result__snippet").unwrap();

    for result in document.select(&result_selector) {
        let title = result
            .select(&title_selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        let url = result
            .select(&title_selector)
            .next()
            .and_then(|el| el.value().attr("href"))
            .map(|href| {
                if href.starts_with("//duckduckgo.com/l/?uddg=") {
                    href.split("uddg=")
                        .nth(1)
                        .and_then(|s| s.split('&').next())
                        .and_then(|encoded| urlencoding::decode(encoded).ok())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| href.to_string())
                } else {
                    href.to_string()
                }
            })
            .unwrap_or_default();

        let snippet = result
            .select(&snippet_selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        if !title.is_empty() && !url.is_empty() {
            results.push(SearchResult {
                title,
                url,
                snippet,
            });
        }
    }

    results
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations: Tool Router
//--------------------------------------------------------------------------------------------------

#[tool_router]
impl Server {
    /// Fetches content from a URL and converts it to markdown.
    ///
    /// Supports HTML pages (converted to markdown) and plain text.
    /// HTTP URLs are automatically upgraded to HTTPS.
    #[tool(
        name = "web__fetch",
        description = "Fetch content from a URL and convert it to markdown."
    )]
    async fn web_fetch(
        &self,
        params: Parameters<WebFetchInput>,
    ) -> Result<Json<WebFetchOutput>, McpError> {
        let input: WebFetchInput = params.0;

        // Validate and normalize URL
        let url = validate_url(&input.url).map_err(to_mcp_error)?;

        // Configure timeout
        let timeout_ms = input.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
        let timeout = Duration::from_millis(timeout_ms);

        // Configure max length
        let max_length = input
            .max_length
            .unwrap_or(DEFAULT_MAX_LENGTH)
            .min(MAX_ALLOWED_LENGTH);

        // Make the request
        let response = self
            .client
            .get(url.as_str())
            .timeout(timeout)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    to_mcp_error(WebError::Timeout(timeout_ms))
                } else if e.is_redirect() {
                    to_mcp_error(WebError::TooManyRedirects(MAX_REDIRECTS))
                } else {
                    to_mcp_error(WebError::RequestFailed(e.to_string()))
                }
            })?;

        let status = response.status().as_u16();
        let final_url = response.url().to_string();

        // Get content type
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(';').next().unwrap_or(s).trim().to_string())
            .unwrap_or_else(|| "text/plain".to_string());

        // Check content length if provided
        if let Some(content_length) = response.content_length() {
            if content_length as usize > max_length {
                return Err(to_mcp_error(WebError::ContentTooLarge {
                    size: content_length as usize,
                    max: max_length,
                }));
            }
        }

        // Get response body
        let bytes = response
            .bytes()
            .await
            .map_err(|e| to_mcp_error(WebError::RequestFailed(e.to_string())))?;

        // Check actual size
        let truncated = bytes.len() > max_length;
        let bytes = if truncated {
            &bytes[..max_length]
        } else {
            &bytes[..]
        };

        // Convert to string
        let text = String::from_utf8_lossy(bytes).to_string();

        // Convert HTML to markdown if applicable
        let content = if content_type.contains("html") {
            html_to_markdown(&text)
        } else {
            text
        };

        Ok(Json(WebFetchOutput {
            content,
            final_url,
            status,
            content_type,
            truncated,
        }))
    }

    /// Searches the web using the best available provider.
    ///
    /// Provider priority: Brave > Tavily > SerpAPI > DuckDuckGo (fallback).
    /// Configure via environment variables:
    /// - BRAVE_SEARCH_API_KEY
    /// - TAVILY_API_KEY
    /// - SERPAPI_API_KEY
    #[tool(
        name = "web__search",
        description = "Search the web and return results with titles, URLs, and snippets."
    )]
    async fn web_search(
        &self,
        params: Parameters<WebSearchInput>,
    ) -> Result<Json<WebSearchOutput>, McpError> {
        let input: WebSearchInput = params.0;

        // Validate query length
        if input.query.len() < MIN_QUERY_LENGTH {
            return Err(to_mcp_error(WebError::QueryTooShort(MIN_QUERY_LENGTH)));
        }

        // Configure max results
        let max_results = input
            .max_results
            .unwrap_or(DEFAULT_MAX_RESULTS)
            .min(MAX_ALLOWED_RESULTS);

        // Execute search with the detected provider
        let (results, provider_name) = match self.search_provider {
            SearchProvider::Brave => {
                let results = search_brave(&self.client, &input.query, max_results)
                    .await
                    .map_err(to_mcp_error)?;
                (results, "Brave Search")
            }
            SearchProvider::Tavily => {
                let results = search_tavily(&self.client, &input.query, max_results)
                    .await
                    .map_err(to_mcp_error)?;
                (results, "Tavily")
            }
            SearchProvider::SerpApi => {
                let results = search_serpapi(&self.client, &input.query, max_results)
                    .await
                    .map_err(to_mcp_error)?;
                (results, "SerpAPI")
            }
            SearchProvider::DuckDuckGo => {
                let results = search_duckduckgo(&self.client, &input.query)
                    .await
                    .map_err(to_mcp_error)?;
                (results, "DuckDuckGo")
            }
        };

        // Apply domain filters
        let results = filter_results(
            results,
            &input.allowed_domains,
            &input.blocked_domains,
            max_results,
        );

        let count = results.len();

        Ok(Json(WebSearchOutput {
            results,
            count,
            provider: provider_name.to_string(),
        }))
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
            instructions: None,
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== URL validation tests ====================

    #[test]
    fn test_validate_url_valid_https() {
        let result = validate_url("https://example.com");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_str(), "https://example.com/");
    }

    #[test]
    fn test_validate_url_http_upgrades_to_https() {
        let result = validate_url("http://example.com");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().scheme(), "https");
    }

    #[test]
    fn test_validate_url_invalid() {
        let result = validate_url("not-a-url");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), WebError::InvalidUrl(_)));
    }

    #[test]
    fn test_validate_url_unsupported_scheme() {
        let result = validate_url("ftp://example.com");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), WebError::InvalidUrl(_)));
    }

    // ==================== HTML to markdown tests ====================

    #[test]
    fn test_html_to_markdown_simple() {
        let html = "<h1>Title</h1><p>Hello world</p>";
        let md = html_to_markdown(html);
        assert!(md.contains("Title"));
        assert!(md.contains("Hello world"));
    }

    #[test]
    fn test_html_to_markdown_links() {
        let html = r#"<a href="https://example.com">Link</a>"#;
        let md = html_to_markdown(html);
        assert!(md.contains("Link"));
        assert!(md.contains("https://example.com"));
    }

    // ==================== Domain matching tests ====================

    #[test]
    fn test_domain_matches_exact() {
        let domains = vec!["example.com".to_string()];
        assert!(domain_matches("https://example.com/page", &domains));
    }

    #[test]
    fn test_domain_matches_subdomain() {
        let domains = vec!["example.com".to_string()];
        assert!(domain_matches("https://sub.example.com/page", &domains));
    }

    #[test]
    fn test_domain_matches_no_match() {
        let domains = vec!["example.com".to_string()];
        assert!(!domain_matches("https://other.com/page", &domains));
    }

    #[test]
    fn test_domain_matches_invalid_url() {
        let domains = vec!["example.com".to_string()];
        assert!(!domain_matches("not-a-url", &domains));
    }

    // ==================== Search provider detection tests ====================

    #[test]
    fn test_search_provider_detect_none() {
        // Clear env vars for test
        // SAFETY: Tests run single-threaded, no concurrent access to env vars
        unsafe {
            env::remove_var("BRAVE_SEARCH_API_KEY");
            env::remove_var("TAVILY_API_KEY");
            env::remove_var("SERPAPI_API_KEY");
        }

        let provider = SearchProvider::detect();
        assert_eq!(provider, SearchProvider::DuckDuckGo);
    }

    #[test]
    fn test_search_provider_names() {
        assert_eq!(SearchProvider::Brave.name(), "Brave Search");
        assert_eq!(SearchProvider::Tavily.name(), "Tavily");
        assert_eq!(SearchProvider::SerpApi.name(), "SerpAPI");
        assert_eq!(SearchProvider::DuckDuckGo.name(), "DuckDuckGo");
    }

    // ==================== Error code tests ====================

    #[test]
    fn test_error_codes() {
        assert_eq!(WebError::InvalidUrl("test".into()).code(), "INVALID_URL");
        assert_eq!(
            WebError::RequestFailed("test".into()).code(),
            "REQUEST_FAILED"
        );
        assert_eq!(WebError::Timeout(1000).code(), "TIMEOUT");
        assert_eq!(
            WebError::ContentTooLarge { size: 100, max: 50 }.code(),
            "CONTENT_TOO_LARGE"
        );
        assert_eq!(WebError::QueryTooShort(2).code(), "QUERY_TOO_SHORT");
        assert_eq!(WebError::TooManyRedirects(10).code(), "TOO_MANY_REDIRECTS");
        assert_eq!(
            WebError::UnsupportedContentType("test".into()).code(),
            "UNSUPPORTED_CONTENT_TYPE"
        );
        assert_eq!(WebError::HttpError(404).code(), "HTTP_ERROR");
        assert_eq!(
            WebError::SearchProviderError("test".into()).code(),
            "SEARCH_PROVIDER_ERROR"
        );
    }

    // ==================== Server tests ====================

    #[test]
    fn test_server_creation() {
        let server = Server::new();
        assert!(server.tool_router.list_all().len() >= 2);
    }

    #[test]
    fn test_server_default() {
        let server = Server::default();
        assert!(server.tool_router.list_all().len() >= 2);
    }

    // ==================== Input validation tests ====================

    #[test]
    fn test_query_too_short() {
        let err = WebError::QueryTooShort(MIN_QUERY_LENGTH);
        assert!(err.to_string().contains("minimum"));
    }

    // ==================== Filter results tests ====================

    #[test]
    fn test_filter_results_allowed() {
        let results = vec![
            SearchResult {
                title: "A".into(),
                url: "https://example.com".into(),
                snippet: "".into(),
            },
            SearchResult {
                title: "B".into(),
                url: "https://other.com".into(),
                snippet: "".into(),
            },
        ];

        let filtered = filter_results(
            results,
            &Some(vec!["example.com".into()]),
            &None,
            10,
        );

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "A");
    }

    #[test]
    fn test_filter_results_blocked() {
        let results = vec![
            SearchResult {
                title: "A".into(),
                url: "https://example.com".into(),
                snippet: "".into(),
            },
            SearchResult {
                title: "B".into(),
                url: "https://blocked.com".into(),
                snippet: "".into(),
            },
        ];

        let filtered = filter_results(
            results,
            &None,
            &Some(vec!["blocked.com".into()]),
            10,
        );

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "A");
    }
}
