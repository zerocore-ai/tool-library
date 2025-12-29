# tools

A collection of MCPB (MCP Bundle) tools for the Radical ecosystem.

## Overview

This repository contains core MCP servers packaged as MCPB bundles. Each tool provides a set of related operations exposed via the Model Context Protocol (MCP).

### Current Tools

| Tool | Description |
|------|-------------|
| `filesystem` | File operations (read, write, edit, glob, grep) |
| `todolist` | Session-scoped task tracking |
| `system` | System utilities (sleep, datetime, random) |

## Creating a New Tool

### Prerequisites

- Rust toolchain (edition 2024)
- macOS (currently the only supported platform)

### Initialize a New Tool

```bash
rad tool init <tool-name>
```

This creates the basic project structure:

```
<tool-name>/
├── Cargo.toml
├── manifest.json
└── src/
    ├── lib.rs
    └── main.rs
```

### Project Structure

**Cargo.toml**

```toml
[package]
name = "<tool-name>"
version = "0.1.0"
edition = "2024"

[dependencies]
rmcp = { version = "0.12", features = ["server", "macros", "transport-io"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread", "io-std"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
schemars = "1"
anyhow = "1.0"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

**manifest.json**

```json
{
  "manifest_version": "0.3",
  "name": "<tool-name>",
  "version": "0.1.0",
  "description": "Description of your tool",
  "author": {
    "name": "Your Name"
  },
  "server": {
    "type": "binary",
    "entry_point": "target/release/<tool-name>",
    "mcp_config": {
      "command": "${__dirname}/target/release/<tool-name>"
    }
  },
  "compatibility": {
    "platforms": ["darwin"]
  },
  "_meta": {
    "company.superrad.radical": {
      "scripts": {
        "build": "cargo build --release"
      }
    }
  }
}
```

## Conventions

### Tool Naming

Tools follow the naming convention `<toolset>__<tool>`:

```
filesystem__read_file
filesystem__write_file
system__sleep
system__get_datetime
todolist__get
todolist__set
```

This namespacing prevents collisions when multiple toolsets are loaded.

### Structured Output Only

All tools must return structured output using `Json<T>`. Text content fallbacks are not supported.

```rust
use rmcp::Json;

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadFileOutput {
    pub content: String,
    pub size: u64,
}

async fn read_file(&self, params: Parameters<ReadFileInput>) -> Result<Json<ReadFileOutput>, McpError> {
    // ...
    Ok(Json(ReadFileOutput { content, size }))
}
```

### Error Handling

All tools use `McpError` (rmcp's `ErrorData`) for protocol-level errors with structured error codes:

```rust
use rmcp::ErrorData as McpError;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum MyToolError {
    #[error("File not found: {0}")]
    NotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),
}

impl MyToolError {
    pub fn code(&self) -> &'static str {
        match self {
            MyToolError::NotFound(_) => "NOT_FOUND",
            MyToolError::PermissionDenied(_) => "PERMISSION_DENIED",
        }
    }

    pub fn to_mcp_error(&self) -> McpError {
        McpError::invalid_params(self.to_string(), Some(json!({ "code": self.code() })))
    }
}
```

Error responses follow this schema:

```json
{
  "error": {
    "code": -32602,
    "message": "File not found: /path/to/file",
    "data": {
      "code": "NOT_FOUND"
    }
  }
}
```

### Code Organization

Source files are organized into sections:

```rust
use std::sync::Arc;
use rmcp::{...};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

pub const MAX_FILE_SIZE: u64 = 10_485_760;

//--------------------------------------------------------------------------------------------------
// Types: Error
//--------------------------------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum MyToolError {
    // ...
}

//--------------------------------------------------------------------------------------------------
// Types: Input/Output
//--------------------------------------------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolInput {
    // ...
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolOutput {
    // ...
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
        // ...
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations: Tool Router
//--------------------------------------------------------------------------------------------------

#[tool_router]
impl Server {
    #[tool(name = "mytool__operation", description = "...")]
    async fn operation(&self, params: Parameters<ToolInput>) -> Result<Json<ToolOutput>, McpError> {
        // ...
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations: Server Handler
//--------------------------------------------------------------------------------------------------

#[tool_handler]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerInfo {
        // ...
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    // ...
}
```

### Server Entry Point

**src/main.rs**

```rust
use anyhow::Result;
use my_tool::Server;
use rmcp::{ServiceExt, transport::stdio};
use tracing_subscriber::{self, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::DEBUG.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let service = Server::new().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
```

## Configuration

### user_config

User-facing configuration that customizes tool behavior. Defined in the manifest:

```json
{
  "user_config": {
    "type": "object",
    "properties": {
      "max_results": {
        "type": "integer",
        "default": 100
      }
    }
  }
}
```

### system_config

System-level configuration for resources like allowed paths, API keys, etc. Not currently implemented but reserved for future use.

## Building and Testing

```bash
# Build release binary
cargo build --release

# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run
```

## Platform Support

Currently, only macOS (`darwin`) binaries are supported. We plan to expand platform support to include:

- Linux (`linux`)
- Windows (`windows`)

## License

See [LICENSE](LICENSE) for details.
