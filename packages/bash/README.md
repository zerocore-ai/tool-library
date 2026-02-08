# Bash MCP Server

Shell command execution for AI agents. Based on [Claude Code's Bash tool design](https://gist.github.com/bgauryy/0cdb9aa337d01ae5bd0c803943aa36bd).

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

### Using tool CLI (Recommended)

Install from https://github.com/zerocore-ai/tool-cli

```bash
# Build the tool
tool run build /path/to/bash
```

```bash
# Validate the manifest
tool validate /path/to/bash
```

```bash
# Test the tool
tool call /path/to/bash -m bash__exec -p command="echo hello"
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
