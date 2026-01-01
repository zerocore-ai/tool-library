use std::time::Instant;

use chrono::Utc;
use rand::Rng;
use rmcp::{
    ErrorData as McpError,
    handler::server::tool::ToolRouter,
    handler::server::wrapper::Parameters,
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, Json, ServerHandler,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// Maximum sleep duration in milliseconds (5 minutes).
pub const MAX_SLEEP_DURATION_MS: u64 = 300_000;

//--------------------------------------------------------------------------------------------------
// Types: Error
//--------------------------------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum SystemError {
    #[error("Duration exceeds maximum allowed ({MAX_SLEEP_DURATION_MS}ms)")]
    DurationTooLong,

    #[error("min ({min}) must be less than or equal to max ({max})")]
    InvalidRange { min: i64, max: i64 },
}

impl SystemError {
    /// Get the error code for this error variant.
    pub fn code(&self) -> &'static str {
        match self {
            SystemError::DurationTooLong => "DURATION_TOO_LONG",
            SystemError::InvalidRange { .. } => "INVALID_RANGE",
        }
    }

    /// Convert to MCP error with structured data.
    pub fn to_mcp_error(&self) -> McpError {
        McpError::invalid_params(self.to_string(), Some(json!({ "code": self.code() })))
    }
}

//--------------------------------------------------------------------------------------------------
// Types: Sleep
//--------------------------------------------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct SleepInput {
    /// Duration to sleep in milliseconds (0 to 300000).
    pub duration_ms: u64,
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct SleepOutput {
    /// Actual duration slept in milliseconds.
    pub slept_ms: u64,
}

//--------------------------------------------------------------------------------------------------
// Types: Get Datetime
//--------------------------------------------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetDatetimeInput {}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetDatetimeOutput {
    /// UTC timestamp in ISO 8601 format.
    pub iso8601: String,

    /// Unix timestamp in milliseconds.
    pub unix_ms: i64,
}

//--------------------------------------------------------------------------------------------------
// Types: Get Random Integer
//--------------------------------------------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetRandomIntegerInput {
    /// Minimum value (inclusive).
    pub min: i64,

    /// Maximum value (inclusive).
    pub max: i64,
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetRandomIntegerOutput {
    /// Random integer in range [min, max].
    pub value: i64,
}

//--------------------------------------------------------------------------------------------------
// Types: Server
//--------------------------------------------------------------------------------------------------

#[derive(Clone)]
pub struct Server {
    tool_router: ToolRouter<Self>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl Server {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
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
    /// Pauses execution for a specified duration.
    #[tool(
        name = "system__sleep",
        description = "Pause execution for a specified duration in milliseconds."
    )]
    async fn sleep(&self, params: Parameters<SleepInput>) -> Result<Json<SleepOutput>, McpError> {
        let input = params.0;

        if input.duration_ms > MAX_SLEEP_DURATION_MS {
            return Err(SystemError::DurationTooLong.to_mcp_error());
        }

        let start = Instant::now();
        tokio::time::sleep(tokio::time::Duration::from_millis(input.duration_ms)).await;
        let elapsed = start.elapsed().as_millis() as u64;

        Ok(Json(SleepOutput { slept_ms: elapsed }))
    }

    /// Returns the current UTC date and time.
    #[tool(
        name = "system__get_datetime",
        description = "Get the current UTC date and time."
    )]
    async fn get_datetime(
        &self,
        _params: Parameters<GetDatetimeInput>,
    ) -> Result<Json<GetDatetimeOutput>, McpError> {
        let now = Utc::now();

        Ok(Json(GetDatetimeOutput {
            iso8601: now.to_rfc3339(),
            unix_ms: now.timestamp_millis(),
        }))
    }

    /// Generates a cryptographically secure random integer within an inclusive range.
    #[tool(
        name = "system__get_random_integer",
        description = "Generate a random integer within an inclusive range [min, max]."
    )]
    async fn get_random_integer(
        &self,
        params: Parameters<GetRandomIntegerInput>,
    ) -> Result<Json<GetRandomIntegerOutput>, McpError> {
        let input = params.0;

        if input.min > input.max {
            return Err(SystemError::InvalidRange {
                min: input.min,
                max: input.max,
            }
            .to_mcp_error());
        }

        let value = rand::rng().random_range(input.min..=input.max);

        Ok(Json(GetRandomIntegerOutput { value }))
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

    // ==================== Serialization Tests ====================

    #[test]
    fn test_sleep_input_serialization() {
        let input = SleepInput { duration_ms: 1000 };
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"duration_ms\":1000"));
    }

    #[test]
    fn test_sleep_output_serialization() {
        let output = SleepOutput { slept_ms: 1000 };
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("\"slept_ms\":1000"));
    }

    #[test]
    fn test_get_datetime_output_serialization() {
        let output = GetDatetimeOutput {
            iso8601: "2024-01-01T00:00:00Z".to_string(),
            unix_ms: 1704067200000,
        };
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("\"iso8601\":\"2024-01-01T00:00:00Z\""));
        assert!(json.contains("\"unix_ms\":1704067200000"));
    }

    #[test]
    fn test_get_random_integer_input_serialization() {
        let input = GetRandomIntegerInput { min: 1, max: 100 };
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"min\":1"));
        assert!(json.contains("\"max\":100"));
    }

    #[test]
    fn test_get_random_integer_output_serialization() {
        let output = GetRandomIntegerOutput { value: 42 };
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("\"value\":42"));
    }

    // ==================== Server Tests ====================

    #[test]
    fn test_server_new() {
        let _server = Server::new();
    }

    #[test]
    fn test_server_default() {
        let _server = Server::default();
    }

    // ==================== Error Tests ====================

    #[test]
    fn test_system_error_duration_too_long() {
        let err = SystemError::DurationTooLong;
        assert!(err.to_string().contains("300000"));
        assert_eq!(err.code(), "DURATION_TOO_LONG");

        let mcp_err = err.to_mcp_error();
        assert_eq!(mcp_err.message, err.to_string());
    }

    #[test]
    fn test_system_error_invalid_range() {
        let err = SystemError::InvalidRange { min: 100, max: 1 };
        let msg = err.to_string();
        assert!(msg.contains("100"));
        assert!(msg.contains("1"));
        assert_eq!(err.code(), "INVALID_RANGE");

        let mcp_err = err.to_mcp_error();
        assert_eq!(mcp_err.message, err.to_string());
    }

    // ==================== Sleep Functional Tests ====================

    #[tokio::test]
    async fn test_sleep_zero_duration() {
        let server = Server::new();
        let params = Parameters(SleepInput { duration_ms: 0 });
        let result = server.sleep(params).await;

        assert!(result.is_ok());
        let output = result.unwrap().0;
        assert!(output.slept_ms < 50);
    }

    #[tokio::test]
    async fn test_sleep_normal_duration() {
        let server = Server::new();
        let params = Parameters(SleepInput { duration_ms: 50 });
        let result = server.sleep(params).await;

        assert!(result.is_ok());
        let output = result.unwrap().0;
        assert!(output.slept_ms >= 50);
        assert!(output.slept_ms < 150);
    }

    #[tokio::test]
    async fn test_sleep_at_max_boundary() {
        let server = Server::new();
        let params = Parameters(SleepInput {
            duration_ms: MAX_SLEEP_DURATION_MS,
        });

        let start = Instant::now();
        let handle = tokio::spawn(async move { server.sleep(params).await });

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        handle.abort();

        assert!(start.elapsed().as_millis() < 100);
    }

    #[tokio::test]
    async fn test_sleep_exceeds_max_duration() {
        let server = Server::new();
        let params = Parameters(SleepInput {
            duration_ms: MAX_SLEEP_DURATION_MS + 1,
        });
        let result = server.sleep(params).await;

        assert!(result.is_err());
        let Err(err) = result else { panic!("expected error") };
        assert!(err.message.contains("300000"));
        assert_eq!(
            err.data.as_ref().unwrap()["code"].as_str().unwrap(),
            "DURATION_TOO_LONG"
        );
    }

    // ==================== Get Datetime Functional Tests ====================

    #[tokio::test]
    async fn test_get_datetime_returns_valid_iso8601() {
        let server = Server::new();
        let params = Parameters(GetDatetimeInput {});
        let result = server.get_datetime(params).await;

        assert!(result.is_ok());
        let output = result.unwrap().0;

        assert!(output.iso8601.contains('T'));
        assert!(output.iso8601.contains('+') || output.iso8601.ends_with('Z'));

        let parsed = chrono::DateTime::parse_from_rfc3339(&output.iso8601);
        assert!(parsed.is_ok());
    }

    #[tokio::test]
    async fn test_get_datetime_returns_reasonable_unix_timestamp() {
        let server = Server::new();
        let params = Parameters(GetDatetimeInput {});
        let result = server.get_datetime(params).await;

        assert!(result.is_ok());
        let output = result.unwrap().0;

        assert!(output.unix_ms > 1577836800000);
        assert!(output.unix_ms < 4102444800000);
    }

    #[tokio::test]
    async fn test_get_datetime_consistency() {
        let server = Server::new();
        let params = Parameters(GetDatetimeInput {});
        let result = server.get_datetime(params).await;

        assert!(result.is_ok());
        let output = result.unwrap().0;

        let parsed = chrono::DateTime::parse_from_rfc3339(&output.iso8601).unwrap();
        let parsed_ms = parsed.timestamp_millis();

        assert!((parsed_ms - output.unix_ms).abs() < 1000);
    }

    // ==================== Get Random Integer Functional Tests ====================

    #[tokio::test]
    async fn test_get_random_integer_normal_range() {
        let server = Server::new();
        let params = Parameters(GetRandomIntegerInput { min: 1, max: 100 });
        let result = server.get_random_integer(params).await;

        assert!(result.is_ok());
        let output = result.unwrap().0;
        assert!(output.value >= 1);
        assert!(output.value <= 100);
    }

    #[tokio::test]
    async fn test_get_random_integer_same_min_max() {
        let server = Server::new();
        let params = Parameters(GetRandomIntegerInput { min: 42, max: 42 });
        let result = server.get_random_integer(params).await;

        assert!(result.is_ok());
        let output = result.unwrap().0;
        assert_eq!(output.value, 42);
    }

    #[tokio::test]
    async fn test_get_random_integer_invalid_range() {
        let server = Server::new();
        let params = Parameters(GetRandomIntegerInput { min: 100, max: 1 });
        let result = server.get_random_integer(params).await;

        assert!(result.is_err());
        let Err(err) = result else { panic!("expected error") };
        assert!(err.message.contains("100"));
        assert!(err.message.contains("1"));
        assert_eq!(
            err.data.as_ref().unwrap()["code"].as_str().unwrap(),
            "INVALID_RANGE"
        );
    }

    #[tokio::test]
    async fn test_get_random_integer_negative_range() {
        let server = Server::new();
        let params = Parameters(GetRandomIntegerInput { min: -100, max: -1 });
        let result = server.get_random_integer(params).await;

        assert!(result.is_ok());
        let output = result.unwrap().0;
        assert!(output.value >= -100);
        assert!(output.value <= -1);
    }

    #[tokio::test]
    async fn test_get_random_integer_spans_zero() {
        let server = Server::new();
        let params = Parameters(GetRandomIntegerInput { min: -50, max: 50 });
        let result = server.get_random_integer(params).await;

        assert!(result.is_ok());
        let output = result.unwrap().0;
        assert!(output.value >= -50);
        assert!(output.value <= 50);
    }

    #[tokio::test]
    async fn test_get_random_integer_large_range() {
        let server = Server::new();
        let params = Parameters(GetRandomIntegerInput {
            min: i64::MIN / 2,
            max: i64::MAX / 2,
        });
        let result = server.get_random_integer(params).await;

        assert!(result.is_ok());
        let output = result.unwrap().0;
        assert!(output.value >= i64::MIN / 2);
        assert!(output.value <= i64::MAX / 2);
    }

    #[tokio::test]
    async fn test_get_random_integer_distribution() {
        let server = Server::new();
        let mut values = std::collections::HashSet::new();

        for _ in 0..20 {
            let params = Parameters(GetRandomIntegerInput { min: 1, max: 1000 });
            let result = server.get_random_integer(params).await.unwrap();
            values.insert(result.0.value);
        }

        assert!(values.len() > 1, "Random values should vary");
    }
}
