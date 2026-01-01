# Todolist MCP Server

An MCP server providing session-scoped task tracking for AI agents. Maintains a simple todo list that persists for the duration of the server session.

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

### Using rad CLI (Recommended)

```bash
# Build the tool
rad tool run build /path/to/todolist

# Validate the manifest
rad tool validate /path/to/todolist

# Get current todos
rad tool call /path/to/todolist -m todolist__get

# Set todos (using --json for complex input)
rad tool call /path/to/todolist -m todolist__set --json '{
  "todos": [
    {"content": "Implement feature X", "status": "in_progress", "activeForm": "Implementing feature X"},
    {"content": "Write tests", "status": "pending", "activeForm": "Writing tests"}
  ]
}'
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
