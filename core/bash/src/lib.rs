use std::path::Path;
use std::time::Instant;

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
use tokio::process::Command;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// Default timeout in milliseconds (2 minutes).
pub const DEFAULT_TIMEOUT_MS: u64 = 120_000;

/// Maximum timeout in milliseconds (10 minutes).
pub const MAX_TIMEOUT_MS: u64 = 600_000;

/// Maximum output size in characters per stream.
pub const MAX_OUTPUT_SIZE: usize = 30_000;

//--------------------------------------------------------------------------------------------------
// Types: Error
//--------------------------------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum BashError {
    #[error("Command cannot be empty")]
    EmptyCommand,

    #[error("Timeout exceeds maximum allowed ({MAX_TIMEOUT_MS}ms): {0}ms")]
    TimeoutTooLong(u64),

    #[error("Command timed out after {0}ms")]
    Timeout(u64),

    #[error("Failed to spawn process: {0}")]
    SpawnFailed(String),

    #[error("Working directory not found: {0}")]
    DirectoryNotFound(String),

    #[error("Working directory not accessible: {0}")]
    DirectoryNotAccessible(String),

    #[error("I/O error: {0}")]
    IoError(String),
}

impl BashError {
    /// Get the error code for this error variant.
    pub fn code(&self) -> &'static str {
        match self {
            BashError::EmptyCommand => "EMPTY_COMMAND",
            BashError::TimeoutTooLong(_) => "TIMEOUT_TOO_LONG",
            BashError::Timeout(_) => "TIMEOUT",
            BashError::SpawnFailed(_) => "SPAWN_FAILED",
            BashError::DirectoryNotFound(_) => "DIRECTORY_NOT_FOUND",
            BashError::DirectoryNotAccessible(_) => "DIRECTORY_NOT_ACCESSIBLE",
            BashError::IoError(_) => "IO_ERROR",
        }
    }

    /// Convert to MCP error with structured data.
    pub fn to_mcp_error(&self) -> McpError {
        McpError::invalid_params(self.to_string(), Some(json!({ "code": self.code() })))
    }
}

//--------------------------------------------------------------------------------------------------
// Types: Exec
//--------------------------------------------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExecInput {
    /// The shell command to execute.
    pub command: String,

    /// Short description (5-10 words) of what the command does.
    #[serde(default)]
    pub description: Option<String>,

    /// Timeout in milliseconds. Default: 120000, Max: 600000.
    #[serde(default)]
    pub timeout_ms: Option<u64>,

    /// Working directory for command execution.
    #[serde(default)]
    pub working_directory: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExecOutput {
    /// Standard output from the command.
    pub stdout: String,

    /// Standard error from the command.
    pub stderr: String,

    /// Exit code of the command (0 = success).
    pub exit_code: i32,

    /// Whether stdout was truncated due to size limits.
    pub stdout_truncated: bool,

    /// Whether stderr was truncated due to size limits.
    pub stderr_truncated: bool,

    /// Actual execution duration in milliseconds.
    pub duration_ms: u64,
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
// Functions: Helpers
//--------------------------------------------------------------------------------------------------

/// Truncate a string to the maximum output size, keeping the tail.
fn truncate_output(output: String) -> (String, bool) {
    if output.len() <= MAX_OUTPUT_SIZE {
        (output, false)
    } else {
        let truncated = output
            .chars()
            .rev()
            .take(MAX_OUTPUT_SIZE)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        (truncated, true)
    }
}

/// Validate the working directory exists and is accessible.
fn validate_working_directory(path: &str) -> Result<(), BashError> {
    let path = Path::new(path);

    if !path.exists() {
        return Err(BashError::DirectoryNotFound(path.display().to_string()));
    }

    if !path.is_dir() {
        return Err(BashError::DirectoryNotAccessible(format!(
            "{} is not a directory",
            path.display()
        )));
    }

    // Check if we can read the directory
    match std::fs::read_dir(path) {
        Ok(_) => Ok(()),
        Err(_) => Err(BashError::DirectoryNotAccessible(path.display().to_string())),
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations: Tool Router
//--------------------------------------------------------------------------------------------------

#[tool_router]
impl Server {
    /// Execute a shell command.
    #[tool(
        name = "bash__exec",
        description = "Execute a shell command and return its output."
    )]
    async fn exec(&self, params: Parameters<ExecInput>) -> Result<Json<ExecOutput>, McpError> {
        let input = params.0;

        // Validate command is not empty
        if input.command.trim().is_empty() {
            return Err(BashError::EmptyCommand.to_mcp_error());
        }

        // Validate and apply timeout
        let timeout_ms = input.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
        if timeout_ms > MAX_TIMEOUT_MS {
            return Err(BashError::TimeoutTooLong(timeout_ms).to_mcp_error());
        }

        // Validate working directory if provided
        if let Some(ref dir) = input.working_directory {
            validate_working_directory(dir).map_err(|e| e.to_mcp_error())?;
        }

        // Build the command
        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c").arg(&input.command);

        if let Some(ref dir) = input.working_directory {
            cmd.current_dir(dir);
        }

        // Execute with timeout
        let start = Instant::now();
        let timeout_duration = tokio::time::Duration::from_millis(timeout_ms);

        let output = match tokio::time::timeout(timeout_duration, cmd.output()).await {
            Ok(result) => result.map_err(|e| BashError::SpawnFailed(e.to_string()).to_mcp_error())?,
            Err(_) => {
                return Err(BashError::Timeout(timeout_ms).to_mcp_error());
            }
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        // Process output
        let stdout_raw =
            String::from_utf8_lossy(&output.stdout).to_string();
        let stderr_raw =
            String::from_utf8_lossy(&output.stderr).to_string();

        let (stdout, stdout_truncated) = truncate_output(stdout_raw);
        let (stderr, stderr_truncated) = truncate_output(stderr_raw);

        let exit_code = output.status.code().unwrap_or(-1);

        Ok(Json(ExecOutput {
            stdout,
            stderr,
            exit_code,
            stdout_truncated,
            stderr_truncated,
            duration_ms,
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

    // ==================== Serialization Tests ====================

    #[test]
    fn test_exec_input_serialization_full() {
        let input = ExecInput {
            command: "echo hello".to_string(),
            description: Some("Print hello".to_string()),
            timeout_ms: Some(5000),
            working_directory: Some("/tmp".to_string()),
        };
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"command\":\"echo hello\""));
        assert!(json.contains("\"description\":\"Print hello\""));
        assert!(json.contains("\"timeout_ms\":5000"));
        assert!(json.contains("\"working_directory\":\"/tmp\""));
    }

    #[test]
    fn test_exec_input_serialization_minimal() {
        let input = ExecInput {
            command: "ls".to_string(),
            description: None,
            timeout_ms: None,
            working_directory: None,
        };
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"command\":\"ls\""));
    }

    #[test]
    fn test_exec_input_deserialization_minimal() {
        let json = r#"{"command": "ls"}"#;
        let input: ExecInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.command, "ls");
        assert!(input.description.is_none());
        assert!(input.timeout_ms.is_none());
        assert!(input.working_directory.is_none());
    }

    #[test]
    fn test_exec_output_serialization() {
        let output = ExecOutput {
            stdout: "hello\n".to_string(),
            stderr: "".to_string(),
            exit_code: 0,
            stdout_truncated: false,
            stderr_truncated: false,
            duration_ms: 42,
        };
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("\"stdout\":\"hello\\n\""));
        assert!(json.contains("\"stderr\":\"\""));
        assert!(json.contains("\"exit_code\":0"));
        assert!(json.contains("\"stdout_truncated\":false"));
        assert!(json.contains("\"stderr_truncated\":false"));
        assert!(json.contains("\"duration_ms\":42"));
    }

    // ==================== Error Tests ====================

    #[test]
    fn test_error_empty_command() {
        let err = BashError::EmptyCommand;
        assert_eq!(err.code(), "EMPTY_COMMAND");
        assert!(err.to_string().contains("empty"));

        let mcp_err = err.to_mcp_error();
        assert_eq!(mcp_err.message, err.to_string());
        assert_eq!(
            mcp_err.data.as_ref().unwrap()["code"].as_str().unwrap(),
            "EMPTY_COMMAND"
        );
    }

    #[test]
    fn test_error_timeout_too_long() {
        let err = BashError::TimeoutTooLong(999999);
        assert_eq!(err.code(), "TIMEOUT_TOO_LONG");
        assert!(err.to_string().contains("999999"));
        assert!(err.to_string().contains("600000"));
    }

    #[test]
    fn test_error_timeout() {
        let err = BashError::Timeout(5000);
        assert_eq!(err.code(), "TIMEOUT");
        assert!(err.to_string().contains("5000"));
    }

    #[test]
    fn test_error_spawn_failed() {
        let err = BashError::SpawnFailed("permission denied".to_string());
        assert_eq!(err.code(), "SPAWN_FAILED");
        assert!(err.to_string().contains("permission denied"));
    }

    #[test]
    fn test_error_directory_not_found() {
        let err = BashError::DirectoryNotFound("/nonexistent".to_string());
        assert_eq!(err.code(), "DIRECTORY_NOT_FOUND");
        assert!(err.to_string().contains("/nonexistent"));
    }

    #[test]
    fn test_error_directory_not_accessible() {
        let err = BashError::DirectoryNotAccessible("/root/secret".to_string());
        assert_eq!(err.code(), "DIRECTORY_NOT_ACCESSIBLE");
        assert!(err.to_string().contains("/root/secret"));
    }

    #[test]
    fn test_error_io_error() {
        let err = BashError::IoError("broken pipe".to_string());
        assert_eq!(err.code(), "IO_ERROR");
        assert!(err.to_string().contains("broken pipe"));
    }

    // ==================== Helper Function Tests ====================

    #[test]
    fn test_truncate_output_under_limit() {
        let input = "hello world".to_string();
        let (output, truncated) = truncate_output(input.clone());
        assert_eq!(output, input);
        assert!(!truncated);
    }

    #[test]
    fn test_truncate_output_at_limit() {
        let input = "x".repeat(MAX_OUTPUT_SIZE);
        let (output, truncated) = truncate_output(input.clone());
        assert_eq!(output, input);
        assert!(!truncated);
    }

    #[test]
    fn test_truncate_output_over_limit() {
        let input = "x".repeat(MAX_OUTPUT_SIZE + 100);
        let (output, truncated) = truncate_output(input.clone());
        assert_eq!(output.len(), MAX_OUTPUT_SIZE);
        assert!(truncated);
        // Should keep the tail
        assert!(output.ends_with("xxx"));
    }

    #[test]
    fn test_validate_working_directory_exists() {
        let result = validate_working_directory("/tmp");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_working_directory_not_found() {
        let result = validate_working_directory("/nonexistent_dir_12345");
        assert!(matches!(result, Err(BashError::DirectoryNotFound(_))));
    }

    #[test]
    fn test_validate_working_directory_not_a_directory() {
        // /etc/passwd exists but is a file, not a directory
        let result = validate_working_directory("/etc/passwd");
        assert!(matches!(result, Err(BashError::DirectoryNotAccessible(_))));
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

    // ==================== Exec Functional Tests ====================

    #[tokio::test]
    async fn test_exec_simple_command() {
        let server = Server::new();
        let params = Parameters(ExecInput {
            command: "echo hello".to_string(),
            description: None,
            timeout_ms: None,
            working_directory: None,
        });

        let result = server.exec(params).await;
        assert!(result.is_ok());

        let output = result.unwrap().0;
        assert_eq!(output.stdout.trim(), "hello");
        assert!(output.stderr.is_empty());
        assert_eq!(output.exit_code, 0);
        assert!(!output.stdout_truncated);
        assert!(!output.stderr_truncated);
        assert!(output.duration_ms < 5000);
    }

    #[tokio::test]
    async fn test_exec_with_description() {
        let server = Server::new();
        let params = Parameters(ExecInput {
            command: "echo test".to_string(),
            description: Some("Print test message".to_string()),
            timeout_ms: None,
            working_directory: None,
        });

        let result = server.exec(params).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().0.stdout.trim(), "test");
    }

    #[tokio::test]
    async fn test_exec_empty_command() {
        let server = Server::new();
        let params = Parameters(ExecInput {
            command: "".to_string(),
            description: None,
            timeout_ms: None,
            working_directory: None,
        });

        let result = server.exec(params).await;
        let Err(err) = result else {
            panic!("Expected error");
        };
        assert_eq!(
            err.data.as_ref().unwrap()["code"].as_str().unwrap(),
            "EMPTY_COMMAND"
        );
    }

    #[tokio::test]
    async fn test_exec_whitespace_only_command() {
        let server = Server::new();
        let params = Parameters(ExecInput {
            command: "   \t\n  ".to_string(),
            description: None,
            timeout_ms: None,
            working_directory: None,
        });

        let result = server.exec(params).await;
        let Err(err) = result else {
            panic!("Expected error");
        };
        assert_eq!(
            err.data.as_ref().unwrap()["code"].as_str().unwrap(),
            "EMPTY_COMMAND"
        );
    }

    #[tokio::test]
    async fn test_exec_stderr_output() {
        let server = Server::new();
        let params = Parameters(ExecInput {
            command: "echo error >&2".to_string(),
            description: None,
            timeout_ms: None,
            working_directory: None,
        });

        let result = server.exec(params).await;
        assert!(result.is_ok());

        let output = result.unwrap().0;
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr.trim(), "error");
        assert_eq!(output.exit_code, 0);
    }

    #[tokio::test]
    async fn test_exec_mixed_stdout_stderr() {
        let server = Server::new();
        let params = Parameters(ExecInput {
            command: "echo stdout; echo stderr >&2".to_string(),
            description: None,
            timeout_ms: None,
            working_directory: None,
        });

        let result = server.exec(params).await;
        assert!(result.is_ok());

        let output = result.unwrap().0;
        assert_eq!(output.stdout.trim(), "stdout");
        assert_eq!(output.stderr.trim(), "stderr");
    }

    #[tokio::test]
    async fn test_exec_nonzero_exit_code() {
        let server = Server::new();
        let params = Parameters(ExecInput {
            command: "exit 42".to_string(),
            description: None,
            timeout_ms: None,
            working_directory: None,
        });

        let result = server.exec(params).await;
        assert!(result.is_ok());

        let output = result.unwrap().0;
        assert_eq!(output.exit_code, 42);
    }

    #[tokio::test]
    async fn test_exec_command_not_found() {
        let server = Server::new();
        let params = Parameters(ExecInput {
            command: "nonexistent_command_12345".to_string(),
            description: None,
            timeout_ms: None,
            working_directory: None,
        });

        let result = server.exec(params).await;
        assert!(result.is_ok());

        let output = result.unwrap().0;
        assert_ne!(output.exit_code, 0);
        assert!(!output.stderr.is_empty());
    }

    #[tokio::test]
    async fn test_exec_with_working_directory() {
        let server = Server::new();
        let params = Parameters(ExecInput {
            command: "pwd".to_string(),
            description: None,
            timeout_ms: None,
            working_directory: Some("/tmp".to_string()),
        });

        let result = server.exec(params).await;
        assert!(result.is_ok());

        let output = result.unwrap().0;
        // On macOS, /tmp is a symlink to /private/tmp
        assert!(
            output.stdout.trim() == "/tmp" || output.stdout.trim() == "/private/tmp",
            "Expected /tmp or /private/tmp, got: {}",
            output.stdout.trim()
        );
    }

    #[tokio::test]
    async fn test_exec_invalid_working_directory() {
        let server = Server::new();
        let params = Parameters(ExecInput {
            command: "pwd".to_string(),
            description: None,
            timeout_ms: None,
            working_directory: Some("/nonexistent_dir_12345".to_string()),
        });

        let result = server.exec(params).await;
        let Err(err) = result else {
            panic!("Expected error");
        };
        assert_eq!(
            err.data.as_ref().unwrap()["code"].as_str().unwrap(),
            "DIRECTORY_NOT_FOUND"
        );
    }

    #[tokio::test]
    async fn test_exec_timeout_too_long() {
        let server = Server::new();
        let params = Parameters(ExecInput {
            command: "echo hello".to_string(),
            description: None,
            timeout_ms: Some(MAX_TIMEOUT_MS + 1),
            working_directory: None,
        });

        let result = server.exec(params).await;
        let Err(err) = result else {
            panic!("Expected error");
        };
        assert_eq!(
            err.data.as_ref().unwrap()["code"].as_str().unwrap(),
            "TIMEOUT_TOO_LONG"
        );
    }

    #[tokio::test]
    async fn test_exec_timeout_at_max() {
        let server = Server::new();
        let params = Parameters(ExecInput {
            command: "echo hello".to_string(),
            description: None,
            timeout_ms: Some(MAX_TIMEOUT_MS),
            working_directory: None,
        });

        let result = server.exec(params).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_exec_custom_timeout() {
        let server = Server::new();
        let params = Parameters(ExecInput {
            command: "echo hello".to_string(),
            description: None,
            timeout_ms: Some(5000),
            working_directory: None,
        });

        let result = server.exec(params).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_exec_timeout_triggered() {
        let server = Server::new();
        let params = Parameters(ExecInput {
            command: "sleep 10".to_string(),
            description: None,
            timeout_ms: Some(100), // 100ms timeout
            working_directory: None,
        });

        let result = server.exec(params).await;
        let Err(err) = result else {
            panic!("Expected error");
        };
        assert_eq!(
            err.data.as_ref().unwrap()["code"].as_str().unwrap(),
            "TIMEOUT"
        );
    }

    #[tokio::test]
    async fn test_exec_multiline_output() {
        let server = Server::new();
        let params = Parameters(ExecInput {
            command: "echo 'line1\nline2\nline3'".to_string(),
            description: None,
            timeout_ms: None,
            working_directory: None,
        });

        let result = server.exec(params).await;
        assert!(result.is_ok());

        let output = result.unwrap().0;
        assert!(output.stdout.contains("line1"));
        assert!(output.stdout.contains("line2"));
        assert!(output.stdout.contains("line3"));
    }

    #[tokio::test]
    async fn test_exec_special_characters() {
        let server = Server::new();
        let params = Parameters(ExecInput {
            command: r#"echo "hello 'world' \"quoted\"""#.to_string(),
            description: None,
            timeout_ms: None,
            working_directory: None,
        });

        let result = server.exec(params).await;
        assert!(result.is_ok());

        let output = result.unwrap().0;
        assert!(output.stdout.contains("hello"));
        assert!(output.stdout.contains("world"));
    }

    #[tokio::test]
    async fn test_exec_pipe_command() {
        let server = Server::new();
        let params = Parameters(ExecInput {
            command: "echo 'hello world' | tr 'a-z' 'A-Z'".to_string(),
            description: None,
            timeout_ms: None,
            working_directory: None,
        });

        let result = server.exec(params).await;
        assert!(result.is_ok());

        let output = result.unwrap().0;
        assert_eq!(output.stdout.trim(), "HELLO WORLD");
    }

    #[tokio::test]
    async fn test_exec_chained_commands() {
        let server = Server::new();
        let params = Parameters(ExecInput {
            command: "echo first && echo second".to_string(),
            description: None,
            timeout_ms: None,
            working_directory: None,
        });

        let result = server.exec(params).await;
        assert!(result.is_ok());

        let output = result.unwrap().0;
        assert!(output.stdout.contains("first"));
        assert!(output.stdout.contains("second"));
    }

    #[tokio::test]
    async fn test_exec_environment_variable() {
        let server = Server::new();
        let params = Parameters(ExecInput {
            command: "MY_VAR=test && echo $MY_VAR".to_string(),
            description: None,
            timeout_ms: None,
            working_directory: None,
        });

        let result = server.exec(params).await;
        assert!(result.is_ok());

        let output = result.unwrap().0;
        assert_eq!(output.stdout.trim(), "test");
    }

    #[tokio::test]
    async fn test_exec_duration_tracking() {
        let server = Server::new();
        let params = Parameters(ExecInput {
            command: "sleep 0.1".to_string(),
            description: None,
            timeout_ms: None,
            working_directory: None,
        });

        let result = server.exec(params).await;
        assert!(result.is_ok());

        let output = result.unwrap().0;
        // Should be at least 100ms
        assert!(output.duration_ms >= 90);
        // But not too long
        assert!(output.duration_ms < 1000);
    }

    #[tokio::test]
    async fn test_exec_large_output_truncation() {
        let server = Server::new();
        // Generate output larger than MAX_OUTPUT_SIZE
        let repeat_count = MAX_OUTPUT_SIZE + 1000;
        let params = Parameters(ExecInput {
            command: format!("head -c {} /dev/zero | tr '\\0' 'x'", repeat_count),
            description: None,
            timeout_ms: None,
            working_directory: None,
        });

        let result = server.exec(params).await;
        assert!(result.is_ok());

        let output = result.unwrap().0;
        assert!(output.stdout_truncated);
        assert_eq!(output.stdout.len(), MAX_OUTPUT_SIZE);
    }

    #[tokio::test]
    async fn test_exec_binary_output_handling() {
        let server = Server::new();
        let params = Parameters(ExecInput {
            command: "printf '\\x00\\x01\\x02hello'".to_string(),
            description: None,
            timeout_ms: None,
            working_directory: None,
        });

        let result = server.exec(params).await;
        assert!(result.is_ok());

        let output = result.unwrap().0;
        // Binary bytes are replaced with replacement character
        assert!(output.stdout.contains("hello"));
    }

    // ==================== Integration Tests ====================

    #[tokio::test]
    async fn test_exec_with_tempdir() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_str().unwrap();

        let server = Server::new();

        // Create a file
        let params = Parameters(ExecInput {
            command: "echo 'test content' > testfile.txt".to_string(),
            description: None,
            timeout_ms: None,
            working_directory: Some(temp_path.to_string()),
        });
        let result = server.exec(params).await;
        assert!(result.is_ok());

        // Read the file back
        let params = Parameters(ExecInput {
            command: "cat testfile.txt".to_string(),
            description: None,
            timeout_ms: None,
            working_directory: Some(temp_path.to_string()),
        });
        let result = server.exec(params).await;
        assert!(result.is_ok());

        let output = result.unwrap().0;
        assert_eq!(output.stdout.trim(), "test content");
    }
}
