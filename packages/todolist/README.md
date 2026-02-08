# Todolist MCP Server

Session-scoped task tracking for AI agents. Based on [Claude Code's TodoWrite tool design](https://gist.github.com/bgauryy/0cdb9aa337d01ae5bd0c803943aa36bd).

## Tools

### `todolist__get`

Get the current state of the todo list.

**Input:** None

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| `todos` | array | List of todo items |
| `summary` | object | Summary with `total`, `pending`, `in_progress`, `completed` counts |

Each todo item:
| Field | Type | Description |
|-------|------|-------------|
| `content` | string | Task description in imperative form (e.g., "Fix authentication bug") |
| `status` | string | One of: `pending`, `in_progress`, `completed` |
| `activeForm` | string | Task description in present continuous form (e.g., "Fixing authentication bug") |

### `todolist__set`

Replace the entire todo list.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `todos` | array | Yes | Complete list of todos to set |

Each todo item:
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `content` | string | Yes | Task description in imperative form |
| `status` | string | Yes | One of: `pending`, `in_progress`, `completed` |
| `activeForm` | string | Yes | Task description in present continuous form |

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| `summary` | object | Summary with `total`, `pending`, `in_progress`, `completed` counts |

## Setup

### Using tool CLI (Recommended)

Install from https://github.com/zerocore-ai/tool-cli

```bash
# Build the tool
tool run build /path/to/todolist
```

```bash
# Validate the manifest
tool validate /path/to/todolist
```

```bash
# Get current todos
tool call /path/to/todolist -m todolist__get
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
