# Terminal MCP Server

An MCP server providing PTY-based terminal sessions with full terminal emulation for interactive applications. Supports multiple concurrent sessions, special keys, and TUI programs.

## Tools

### `terminal__create`

Create a new terminal session running any program (shell by default).

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `program` | string | No | Program to run (default: $SHELL or /bin/bash) |
| `args` | array | No | Program arguments |
| `rows` | integer | No | Terminal height in rows (default: 24) |
| `cols` | integer | No | Terminal width in columns (default: 80) |
| `env` | object | No | Additional environment variables |
| `cwd` | string | No | Working directory |
| `wait_ready` | boolean | No | Wait for shell prompt before returning (default: true for shells) |
| `ready_timeout_ms` | integer | No | Timeout for wait_ready in milliseconds (default: 5000) |

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| `session_id` | string | Unique identifier for subsequent operations |
| `pid` | integer | Process ID |
| `program` | string | Program running in the session |
| `dimensions` | object | Terminal dimensions (`rows`, `cols`) |

### `terminal__destroy`

Terminate a terminal session and clean up resources.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `session_id` | string | Yes | Session ID to destroy |
| `force` | boolean | No | Force kill (SIGKILL) instead of graceful SIGTERM |

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| `destroyed` | boolean | Whether the session was destroyed |
| `exit_code` | integer | Exit code of the terminated process |

### `terminal__list`

List all active terminal sessions.

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| `sessions` | array | List of session info objects |
| `count` | integer | Number of active sessions |

### `terminal__send`

Send input (text or special keys) to a terminal session.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `session_id` | string | Yes | Session ID to send input to |
| `text` | string | No | Text to send |
| `key` | string | No | Special key: `up`, `down`, `left`, `right`, `home`, `end`, `pageup`, `pagedown`, `backspace`, `delete`, `insert`, `tab`, `enter`, `escape`, `f1`-`f12` |
| `ctrl` | boolean | No | Ctrl modifier |
| `alt` | boolean | No | Alt modifier |
| `shift` | boolean | No | Shift modifier |
| `bracketed_paste` | string | No | Bracketed paste mode: `auto`, `always`, `never` |
| `read` | object | No | Optional: read output after sending |

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| `sent` | boolean | Whether input was sent |
| `read_result` | object | Read result (if `read` was specified) |

### `terminal__read`

Read output from a terminal session.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `session_id` | string | Yes | Session ID to read from |
| `view` | string | No | View mode: `screen` (visible buffer), `new` (since last read), `scrollback` (history) |
| `format` | string | No | Output format: `plain` (no ANSI) or `raw` (with ANSI) |
| `timeout_ms` | integer | No | Maximum wait time in milliseconds |
| `wait_idle_ms` | integer | No | Wait until no output for N milliseconds |
| `wait_for_prompt` | boolean | No | Wait for shell prompt |
| `offset` | integer | No | Pagination offset for scrollback |
| `limit` | integer | No | Pagination limit for scrollback |

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| `content` | string | Terminal content |
| `lines` | integer | Number of lines |
| `cursor` | object | Cursor position |
| `dimensions` | object | Terminal dimensions |
| `has_new_content` | boolean | Whether new content was received |
| `prompt_detected` | boolean | Whether a prompt was detected |
| `idle` | boolean | Whether the terminal is idle |
| `exited` | boolean | Whether the process exited |
| `exit_code` | integer | Exit code (if exited) |

### `terminal__info`

Get information about a terminal session without reading content.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `session_id` | string | Yes | Session ID to get info for |

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| `session_id` | string | Session ID |
| `program` | string | Program running |
| `args` | array | Program arguments |
| `pid` | integer | Process ID |
| `created_at` | string | Creation timestamp |
| `cursor` | object | Cursor position |
| `dimensions` | object | Terminal dimensions |
| `exited` | boolean | Whether process exited |
| `exit_code` | integer | Exit code (if exited) |
| `healthy` | boolean | Whether session is healthy |
| `cwd` | string | Current working directory |

## Session Attachment

Sessions can be attached to from the command line, allowing humans to observe and interact with AI-controlled terminal sessions in real-time.

Each session creates a Unix socket at `/tmp/terminal/<session_id>.sock` that accepts multiple concurrent connections.

### CLI Commands

```bash
# List active sessions
terminal list

# Attach to a session (supports prefix matching)
terminal attach sess_abc123
terminal attach sess_a  # prefix match

# Show session details
terminal info sess_abc123

# Run MCP server (default when no command specified)
terminal serve
terminal  # equivalent
```

### Attach Mode

When attached:
- See real-time terminal output
- Type to send input to the session
- Press `Ctrl+C` to detach (returns to your shell)
- Multiple users can attach simultaneously

## Configuration

### MCPB User Config

When installed via MCPB, configure defaults through the manifest:

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `default_rows` | integer | 24 | Default terminal height |
| `default_cols` | integer | 80 | Default terminal width |
| `default_shell` | string | $SHELL | Default shell for new sessions |
| `term` | string | xterm-256color | TERM environment variable |
| `scrollback_limit` | integer | 10000 | Max scrollback lines per session |
| `prompt_pattern` | string | `\$\s*$\|#\s*$\|>\s*$` | Regex for prompt detection |
| `max_sessions` | integer | 10 | Maximum concurrent sessions |

## Setup

### Using tool CLI (Recommended)

Install from https://github.com/zerocore-ai/tool-cli

```bash
# Build the tool
tool run build /path/to/terminal
```

```bash
# Validate the manifest
tool validate /path/to/terminal
```

```bash
# Test creating a session
tool call /path/to/terminal -m terminal__create
```

```bash
# List active sessions
tool call /path/to/terminal -m terminal__list
```

### Manual Build

```bash
cargo build --release
```

## Testing

```bash
# Run unit tests
cargo test

# Run integration tests
cargo test --test integration
```

## Platform Support

- macOS (darwin)
- Linux

## License

MIT
