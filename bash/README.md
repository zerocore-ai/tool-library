# Bash MCP Server

An MCP server for executing shell commands with configurable timeout and structured output.

## Tools

### `bash__exec`

Execute a shell command and return its output.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `command` | string | Yes | The shell command to execute |
| `description` | string | No | Short description (5-10 words) of what the command does |
| `timeout_ms` | integer | No | Timeout in milliseconds (default: 120000, max: 600000) |
| `working_directory` | string | No | Working directory for command execution |

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| `stdout` | string | Standard output from the command |
| `stderr` | string | Standard error from the command |
| `exit_code` | integer | Exit code of the command (0 = success) |
| `stdout_truncated` | boolean | Whether stdout was truncated due to size limits |
| `stderr_truncated` | boolean | Whether stderr was truncated due to size limits |
| `duration_ms` | integer | Actual execution duration in milliseconds |

## Setup

### Using rad CLI (Recommended)

```bash
# Build the tool
rad tool run build /path/to/bash

# Validate the manifest
rad tool validate /path/to/bash

# Test the tool
rad tool call /path/to/bash -m bash__exec -p command="echo hello"
```

### Manual Build

```bash
cargo build --release
```

## Testing

```bash
cargo test
```

## License

MIT
