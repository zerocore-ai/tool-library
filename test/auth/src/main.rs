use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use axum::{
    Json, Router,
    body::Body,
    extract::{Form, Query, State},
    http::{Request, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{any_service, get, post},
};
use clap::Parser;
use rand::{Rng, distr::Alphanumeric};
use rmcp::transport::{
    StreamableHttpServerConfig,
    streamable_http_server::{StreamableHttpService, session::local::LocalSessionManager},
};
use serde::{Deserialize, Serialize};
use auth::Server;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tracing::{debug, error, info, warn};
use tracing_subscriber::{self, EnvFilter};
use uuid::Uuid;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "3000")]
    port: u16,
}

/// OAuth client configuration
#[derive(Debug, Clone)]
struct OAuthClientConfig {
    redirect_uri: String,
}

/// Authorization metadata response
#[derive(Debug, Clone, Deserialize, Serialize)]
struct AuthorizationMetadata {
    authorization_endpoint: String,
    token_endpoint: String,
    registration_endpoint: Option<String>,
    issuer: Option<String>,
    jwks_uri: Option<String>,
    scopes_supported: Option<Vec<String>>,
    #[serde(flatten)]
    additional_fields: HashMap<String, serde_json::Value>,
}

/// Protected resource metadata (RFC 9728)
#[derive(Debug, Clone, Serialize)]
struct ProtectedResourceMetadata {
    resource: String,
    authorization_servers: Vec<String>,
    scopes_supported: Option<Vec<String>>,
}

/// Client registration response
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClientRegistrationResponse {
    client_id: String,
    client_secret: Option<String>,
    client_name: Option<String>,
    redirect_uris: Vec<String>,
    #[serde(flatten)]
    additional_fields: HashMap<String, serde_json::Value>,
}

/// Local registration request
#[derive(Debug, Deserialize)]
struct LocalClientRegistrationRequest {
    client_name: String,
    redirect_uris: Vec<String>,
}

/// OAuth store for managing tokens and sessions
#[derive(Clone, Debug)]
struct McpOAuthStore {
    clients: Arc<RwLock<HashMap<String, OAuthClientConfig>>>,
    auth_sessions: Arc<RwLock<HashMap<String, AuthSession>>>,
    access_tokens: Arc<RwLock<HashMap<String, McpAccessToken>>>,
}

/// Combined application state
#[derive(Clone)]
struct AppState {
    oauth_store: Arc<McpOAuthStore>,
    addr: String,
}

/// Auth session record
#[derive(Clone, Debug)]
struct AuthSession {
    client_id: String,
    scope: Option<String>,
    _state: Option<String>,
    _created_at: chrono::DateTime<chrono::Utc>,
    auth_token: Option<AuthToken>,
    /// RFC 8707 resource indicator - stored to verify at token exchange
    resource: Option<String>,
}

/// Auth token record
#[derive(Clone, Debug, Serialize, Deserialize)]
struct AuthToken {
    access_token: String,
    token_type: String,
    expires_in: u64,
    refresh_token: String,
    scope: Option<String>,
}

/// MCP access token record
#[derive(Clone, Debug, Serialize)]
struct McpAccessToken {
    access_token: String,
    token_type: String,
    expires_in: u64,
    refresh_token: String,
    scope: Option<String>,
    auth_token: AuthToken,
    client_id: String,
}

#[derive(Debug, Deserialize)]
struct AuthorizeQuery {
    #[allow(dead_code)]
    response_type: String,
    client_id: String,
    redirect_uri: String,
    scope: Option<String>,
    state: Option<String>,
    /// RFC 8707 resource indicator - canonical URI of the MCP server
    resource: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApprovalForm {
    client_id: String,
    redirect_uri: String,
    scope: String,
    state: String,
    approved: String,
    /// RFC 8707 resource indicator
    resource: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct TokenRequest {
    grant_type: String,
    #[serde(default)]
    code: String,
    #[serde(default)]
    client_id: String,
    #[serde(default)]
    client_secret: String,
    #[serde(default)]
    redirect_uri: String,
    #[serde(default)]
    code_verifier: Option<String>,
    #[serde(default)]
    refresh_token: String,
    /// RFC 8707 resource indicator - must match authorization request
    #[serde(default)]
    resource: Option<String>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl McpOAuthStore {
    fn new() -> Self {
        let mut clients = HashMap::new();
        clients.insert(
            "mcp-client".to_string(),
            OAuthClientConfig {
                redirect_uri: "http://localhost:8080/callback".to_string(),
            },
        );

        Self {
            clients: Arc::new(RwLock::new(clients)),
            auth_sessions: Arc::new(RwLock::new(HashMap::new())),
            access_tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn validate_client(
        &self,
        client_id: &str,
        redirect_uri: &str,
    ) -> Option<OAuthClientConfig> {
        let clients = self.clients.read().await;
        debug!("validate_client: looking for client_id={}, redirect_uri={}", client_id, redirect_uri);
        debug!("registered clients: {:?}", clients.keys().collect::<Vec<_>>());

        if let Some(client) = clients.get(client_id) {
            debug!("found client, stored redirect_uri={}", client.redirect_uri);
            // Allow empty redirect_uri in token request (some clients omit it)
            // or exact match, or registered URI contains the request URI
            if redirect_uri.is_empty()
                || client.redirect_uri == redirect_uri
                || client.redirect_uri.contains(redirect_uri)
            {
                return Some(client.clone());
            }
        }
        None
    }

    async fn create_auth_session(
        &self,
        client_id: String,
        scope: Option<String>,
        state: Option<String>,
        resource: Option<String>,
        session_id: String,
    ) -> String {
        let session = AuthSession {
            client_id,
            scope,
            _state: state,
            _created_at: chrono::Utc::now(),
            auth_token: None,
            resource,
        };

        self.auth_sessions
            .write()
            .await
            .insert(session_id.clone(), session);
        session_id
    }

    /// Get the resource from an auth session (for validation at token exchange)
    async fn get_session_resource(&self, session_id: &str) -> Option<String> {
        self.auth_sessions
            .read()
            .await
            .get(session_id)
            .and_then(|s| s.resource.clone())
    }

    async fn update_auth_session_token(
        &self,
        session_id: &str,
        token: AuthToken,
    ) -> std::result::Result<(), String> {
        let mut sessions = self.auth_sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.auth_token = Some(token);
            Ok(())
        } else {
            Err("Session not found".to_string())
        }
    }

    async fn create_mcp_token(&self, session_id: &str) -> std::result::Result<McpAccessToken, String> {
        let sessions = self.auth_sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            if let Some(auth_token) = &session.auth_token {
                let access_token = format!("mcp-token-{}", Uuid::new_v4());
                let token = McpAccessToken {
                    access_token: access_token.clone(),
                    token_type: "Bearer".to_string(),
                    expires_in: 3600,
                    refresh_token: format!("mcp-refresh-{}", Uuid::new_v4()),
                    scope: session.scope.clone(),
                    auth_token: auth_token.clone(),
                    client_id: session.client_id.clone(),
                };

                self.access_tokens
                    .write()
                    .await
                    .insert(access_token.clone(), token.clone());
                Ok(token)
            } else {
                Err("No third-party token available for session".to_string())
            }
        } else {
            Err("Session not found".to_string())
        }
    }

    async fn validate_token(&self, token: &str) -> Option<McpAccessToken> {
        self.access_tokens.read().await.get(token).cloned()
    }
}

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

fn generate_random_string(length: usize) -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

/// Index page
async fn index(State(state): State<AppState>) -> Html<String> {
    Html(format!(r#"<!DOCTYPE html>
<html>
<head>
    <title>MCP OAuth Test Server</title>
</head>
<body>
    <h1>MCP OAuth Test Server</h1>
    <p>This server implements MCP with OAuth 2.0 authorization.</p>
    <h2>Endpoints</h2>
    <ul>
        <li><code>GET /.well-known/oauth-authorization-server</code> - OAuth metadata</li>
        <li><code>POST /oauth/register</code> - Dynamic client registration</li>
        <li><code>GET /oauth/authorize</code> - Authorization endpoint</li>
        <li><code>POST /oauth/token</code> - Token endpoint</li>
        <li><code>POST /mcp</code> - MCP endpoint (requires Bearer token)</li>
    </ul>
    <h2>Quick Test</h2>
    <p>Get metadata: <code>curl http://{}/\.well-known/oauth-authorization-server</code></p>
</body>
</html>"#, state.addr))
}

/// Protected resource metadata (RFC 9728)
async fn oauth_protected_resource(State(state): State<AppState>) -> impl IntoResponse {
    let addr = &state.addr;
    let metadata = ProtectedResourceMetadata {
        resource: format!("http://{}/mcp", addr),
        authorization_servers: vec![format!("http://{}", addr)],
        scopes_supported: Some(vec!["profile".to_string(), "email".to_string()]),
    };
    debug!("protected resource metadata: {:?}", metadata);
    (StatusCode::OK, Json(metadata))
}

/// OAuth authorization server metadata
async fn oauth_authorization_server(State(state): State<AppState>) -> impl IntoResponse {
    let addr = &state.addr;
    let mut additional_fields = HashMap::new();
    additional_fields.insert(
        "response_types_supported".into(),
        serde_json::Value::Array(vec![serde_json::Value::String("code".into())]),
    );
    additional_fields.insert(
        "code_challenge_methods_supported".into(),
        serde_json::Value::Array(vec![serde_json::Value::String("S256".into())]),
    );

    let metadata = AuthorizationMetadata {
        authorization_endpoint: format!("http://{}/oauth/authorize", addr),
        token_endpoint: format!("http://{}/oauth/token", addr),
        scopes_supported: Some(vec!["profile".to_string(), "email".to_string()]),
        registration_endpoint: Some(format!("http://{}/oauth/register", addr)),
        issuer: Some(addr.clone()),
        jwks_uri: Some(format!("http://{}/oauth/jwks", addr)),
        additional_fields,
    };

    debug!("metadata: {:?}", metadata);
    (StatusCode::OK, Json(metadata))
}

/// OAuth authorize endpoint
async fn oauth_authorize(
    Query(params): Query<AuthorizeQuery>,
    State(app_state): State<AppState>,
) -> impl IntoResponse {
    debug!("doing oauth_authorize");
    debug!("resource parameter: {:?}", params.resource);

    if let Some(_client) = app_state.oauth_store
        .validate_client(&params.client_id, &params.redirect_uri)
        .await
    {
        let scope = params.scope.clone().unwrap_or_default();
        let state = params.state.clone().unwrap_or_default();
        let resource = params.resource.clone().unwrap_or_default();

        Html(format!(r#"<!DOCTYPE html>
<html>
<head>
    <title>Authorize Application</title>
</head>
<body>
    <h1>Authorization Request</h1>
    <p>Application <strong>{}</strong> is requesting access to:</p>
    <ul>
        <li>{}</li>
    </ul>
    <form method="POST" action="/oauth/approve">
        <input type="hidden" name="client_id" value="{}" />
        <input type="hidden" name="redirect_uri" value="{}" />
        <input type="hidden" name="scope" value="{}" />
        <input type="hidden" name="state" value="{}" />
        <input type="hidden" name="resource" value="{}" />
        <button type="submit" name="approved" value="true">Approve</button>
        <button type="submit" name="approved" value="false">Deny</button>
    </form>
</body>
</html>"#, params.client_id, scope, params.client_id, params.redirect_uri, scope, state, resource)).into_response()
    } else {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "invalid_request",
                "error_description": "invalid client id or redirect uri"
            })),
        )
            .into_response()
    }
}

/// Handle approval of authorization
async fn oauth_approve(
    State(app_state): State<AppState>,
    Form(form): Form<ApprovalForm>,
) -> impl IntoResponse {
    let store = &app_state.oauth_store;
    if form.approved != "true" {
        let redirect_url = format!(
            "{}?error=access_denied&error_description={}{}",
            form.redirect_uri,
            "user rejected the authorization request",
            if form.state.is_empty() {
                "".to_string()
            } else {
                format!("&state={}", form.state)
            }
        );
        return Redirect::to(&redirect_url).into_response();
    }

    let session_id = Uuid::new_v4().to_string();
    let auth_code = format!("mcp-code-{}", session_id);

    debug!("Creating auth session with resource: {:?}", form.resource);

    let session_id = store
        .create_auth_session(
            form.client_id.clone(),
            Some(form.scope.clone()),
            Some(form.state.clone()),
            form.resource.clone(),
            session_id.clone(),
        )
        .await;

    let created_token = AuthToken {
        access_token: format!("tp-token-{}", Uuid::new_v4()),
        token_type: "Bearer".to_string(),
        expires_in: 3600,
        refresh_token: format!("tp-refresh-{}", Uuid::new_v4()),
        scope: Some(form.scope),
    };

    if let Err(e) = store
        .update_auth_session_token(&session_id, created_token)
        .await
    {
        error!("Failed to update session token: {}", e);
    }

    let redirect_url = format!(
        "{}?code={}{}",
        form.redirect_uri,
        auth_code,
        if form.state.is_empty() {
            "".to_string()
        } else {
            format!("&state={}", form.state)
        }
    );

    info!("authorization approved, redirecting to: {}", redirect_url);
    Redirect::to(&redirect_url).into_response()
}

/// Extract client_id from Authorization header (Basic auth)
fn extract_client_id_from_auth_header(headers: &axum::http::HeaderMap) -> Option<String> {
    use base64::{Engine, engine::general_purpose::STANDARD};

    let auth_header = headers.get("Authorization")?.to_str().ok()?;
    let stripped = auth_header.strip_prefix("Basic ")?;
    let decoded = String::from_utf8(STANDARD.decode(stripped).ok()?).ok()?;
    // Basic auth format is "client_id:client_secret" - extract just the client_id
    let client_id = decoded.split(':').next()?.to_string();
    if client_id.is_empty() {
        None
    } else {
        Some(client_id)
    }
}

/// Token endpoint
async fn oauth_token(
    State(app_state): State<AppState>,
    request: Request<Body>,
) -> impl IntoResponse {
    let store = &app_state.oauth_store;
    info!("Received token request");

    // Try to extract client_id from Authorization header first
    let header_client_id = extract_client_id_from_auth_header(request.headers());
    if let Some(ref cid) = header_client_id {
        debug!("Found client_id in Authorization header: {}", cid);
    }

    let bytes = match axum::body::to_bytes(request.into_body(), usize::MAX).await {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("can't read request body: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "invalid_request",
                    "error_description": "can't read request body"
                })),
            )
                .into_response();
        }
    };

    let body_str = String::from_utf8_lossy(&bytes);
    info!("request body: {}", body_str);

    let token_req = match serde_urlencoded::from_bytes::<TokenRequest>(&bytes) {
        Ok(form) => {
            info!("successfully parsed form data: {:?}", form);
            form
        }
        Err(e) => {
            error!("can't parse form data: {}", e);
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({
                    "error": "invalid_request",
                    "error_description": format!("can't parse form data: {}", e)
                })),
            )
                .into_response();
        }
    };

    if token_req.grant_type == "refresh_token" {
        warn!("this server only supports authorization_code");
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "unsupported_grant_type",
                "error_description": "only authorization_code is supported"
            })),
        )
            .into_response();
    }

    if token_req.grant_type != "authorization_code" {
        info!("unsupported grant type: {}", token_req.grant_type);
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "unsupported_grant_type",
                "error_description": "only authorization_code is supported"
            })),
        )
            .into_response();
    }

    if !token_req.code.starts_with("mcp-code-") {
        info!("invalid authorization code: {}", token_req.code);
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "invalid_grant",
                "error_description": "invalid authorization code"
            })),
        )
            .into_response();
    }

    // Prefer client_id from Authorization header (Basic auth), then body, then default
    let client_id = header_client_id
        .or_else(|| {
            if token_req.client_id.is_empty() {
                None
            } else {
                Some(token_req.client_id.clone())
            }
        })
        .unwrap_or_else(|| "mcp-client".to_string());

    debug!("Using client_id for token validation: {}", client_id);

    match store
        .validate_client(&client_id, &token_req.redirect_uri)
        .await
    {
        Some(_) => {
            let session_id = token_req.code.replace("mcp-code-", "");
            info!("got session id: {}", session_id);

            // RFC 8707: Validate resource parameter matches what was used in authorization
            let session_resource = store.get_session_resource(&session_id).await;
            debug!(
                "Resource validation - session: {:?}, token_req: {:?}",
                session_resource, token_req.resource
            );

            // If resource was provided during authorization, it must match in token request
            // Empty/None resources are treated as equivalent for backwards compatibility
            let session_resource_normalized = session_resource
                .as_ref()
                .filter(|s| !s.is_empty());
            let token_resource_normalized = token_req.resource
                .as_ref()
                .filter(|s| !s.is_empty());

            if session_resource_normalized != token_resource_normalized {
                warn!(
                    "Resource mismatch - session: {:?}, token: {:?}",
                    session_resource_normalized, token_resource_normalized
                );
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": "invalid_target",
                        "error_description": "resource parameter mismatch between authorization and token requests"
                    })),
                )
                    .into_response();
            }

            match store.create_mcp_token(&session_id).await {
                Ok(token) => {
                    info!("successfully created access token");
                    (
                        StatusCode::OK,
                        Json(serde_json::json!({
                            "access_token": token.access_token,
                            "token_type": token.token_type,
                            "expires_in": token.expires_in,
                            "refresh_token": token.refresh_token,
                            "scope": token.scope,
                        })),
                    )
                        .into_response()
                }
                Err(e) => {
                    error!("failed to create access token: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({
                            "error": "server_error",
                            "error_description": format!("failed to create access token: {}", e)
                        })),
                    )
                        .into_response()
                }
            }
        }
        None => {
            info!(
                "invalid client id or redirect uri: {} / {}",
                client_id, token_req.redirect_uri
            );
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "invalid_client",
                    "error_description": "invalid client id or redirect uri"
                })),
            )
                .into_response()
        }
    }
}

/// Client registration endpoint
async fn oauth_register(
    State(app_state): State<AppState>,
    Json(req): Json<LocalClientRegistrationRequest>,
) -> impl IntoResponse {
    let store = &app_state.oauth_store;
    debug!("register request: {:?}", req);
    if req.redirect_uris.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "invalid_request",
                "error_description": "at least one redirect uri is required"
            })),
        )
            .into_response();
    }

    let client_id = format!("client-{}", Uuid::new_v4());
    let client_secret = generate_random_string(32);

    let client = OAuthClientConfig {
        redirect_uri: req.redirect_uris[0].clone(),
    };

    store
        .clients
        .write()
        .await
        .insert(client_id.clone(), client);

    let response = ClientRegistrationResponse {
        client_id,
        client_secret: Some(client_secret),
        client_name: Some(req.client_name),
        redirect_uris: req.redirect_uris,
        additional_fields: HashMap::new(),
    };

    (StatusCode::CREATED, Json(response)).into_response()
}

/// Token validation middleware - only protects /mcp paths
async fn validate_token_middleware(
    State(app_state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let path = request.uri().path();

    // Only protect /mcp paths
    if !path.starts_with("/mcp") {
        return next.run(request).await;
    }

    debug!("validate_token_middleware for {}", path);

    let addr = &app_state.addr;
    let resource_metadata_url = format!("http://{}/.well-known/oauth-protected-resource", addr);

    // Build WWW-Authenticate header value per RFC 9728
    let www_authenticate = format!(
        r#"Bearer resource_metadata="{}""#,
        resource_metadata_url
    );

    let auth_header = request.headers().get("Authorization");
    let token = match auth_header {
        Some(header) => {
            let header_str = header.to_str().unwrap_or("");
            if let Some(stripped) = header_str.strip_prefix("Bearer ") {
                stripped.to_string()
            } else {
                info!("Invalid auth header format, returning 401");
                return (
                    StatusCode::UNAUTHORIZED,
                    [(header::WWW_AUTHENTICATE, www_authenticate)],
                ).into_response();
            }
        }
        None => {
            info!("No auth header, returning 401");
            return (
                StatusCode::UNAUTHORIZED,
                [(header::WWW_AUTHENTICATE, www_authenticate)],
            ).into_response();
        }
    };

    match app_state.oauth_store.validate_token(&token).await {
        Some(_) => {
            info!("Token valid, proceeding");
            next.run(request).await
        }
        None => {
            info!("Token invalid, returning 401");
            (
                StatusCode::UNAUTHORIZED,
                [(header::WWW_AUTHENTICATE, www_authenticate)],
            ).into_response()
        }
    }
}

/// Request logging middleware
async fn log_request(request: Request<Body>, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    info!("REQUEST: {} {}", method, uri);
    let response = next.run(request).await;
    let status = response.status();
    info!("RESPONSE: {} for {} {}", status, method, uri);
    response
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::DEBUG.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let args = Args::parse();
    let addr = format!("127.0.0.1:{}", args.port);

    let app_state = AppState {
        oauth_store: Arc::new(McpOAuthStore::new()),
        addr: addr.clone(),
    };

    let mcp_service: StreamableHttpService<Server, LocalSessionManager> =
        StreamableHttpService::new(
            || Ok(Server::new()),
            LocalSessionManager::default().into(),
            StreamableHttpServerConfig::default(),
        );

    // CORS layer for OAuth endpoints
    let cors_layer = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // OAuth server routes with CORS - state must be set before merge
    let oauth_server_router = Router::new()
        .route(
            "/.well-known/oauth-authorization-server",
            get(oauth_authorization_server).options(oauth_authorization_server),
        )
        .route(
            "/.well-known/oauth-protected-resource",
            get(oauth_protected_resource).options(oauth_protected_resource),
        )
        .route("/oauth/token", post(oauth_token).options(oauth_token))
        .route(
            "/oauth/register",
            post(oauth_register).options(oauth_register),
        )
        .layer(cors_layer)
        .with_state(app_state.clone());

    // Main router with auth middleware at app level (protects /mcp paths)
    let app = Router::new()
        .route("/", get(index))
        .route("/oauth/authorize", get(oauth_authorize))
        .route("/oauth/approve", post(oauth_approve))
        .merge(oauth_server_router)
        .route("/mcp", any_service(mcp_service))
        .with_state(app_state.clone())
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            validate_token_middleware,
        ))
        .layer(middleware::from_fn(log_request));

    let tcp_listener = tokio::net::TcpListener::bind(&addr).await?;

    eprintln!("auth MCP OAuth server running on http://{}/mcp", addr);

    axum::serve(tcp_listener, app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.unwrap();
        })
        .await?;

    Ok(())
}
