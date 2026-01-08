# Filesystem MCP Server

An MCP server providing file system operations for AI agents including reading, writing, editing, and searching files.

## Tools

### `filesystem__read`

Read a file from the local filesystem.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `file_path` | string | Yes | Absolute path to the file to read |
| `offset` | integer | No | Starting line number (1-indexed). Defaults to 1 |
| `limit` | integer | No | Number of lines to read. Defaults to 2000 |

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| `content` | string | File content with line numbers in `cat -n` format |
| `total_lines` | integer | Total number of lines in the file |
| `start_line` | integer | Starting line number of the returned content |
| `end_line` | integer | Ending line number of the returned content |
| `truncated` | boolean | Whether the file was truncated |

### `filesystem__write`

Write content to a file.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `file_path` | string | Yes | Absolute path to the file to write |
| `content` | string | Yes | Content to write to the file |

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| `bytes_written` | integer | Number of bytes written |

### `filesystem__edit`

Edit a file by replacing exact string matches.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `file_path` | string | Yes | Absolute path to the file to edit |
| `old_string` | string | Yes | The exact string to find and replace |
| `new_string` | string | Yes | The replacement string |
| `replace_all` | boolean | No | If true, replace all occurrences. Defaults to false |

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| `replacements` | integer | Number of replacements made |

### `filesystem__glob`

Find files matching a glob pattern.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `pattern` | string | Yes | Glob pattern to match (e.g., `**/*.rs`, `src/*.ts`) |
| `path` | string | No | Directory to search in. Defaults to current working directory |

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| `files` | array | List of matching file paths |

### `filesystem__grep`

Search file contents using regex patterns.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `pattern` | string | Yes | Regex pattern to search for |
| `path` | string | No | File or directory to search in. Defaults to cwd |
| `glob` | string | No | Glob pattern to filter files (e.g., `*.js`, `*.{ts,tsx}`) |
| `type` | string | No | File type to search (e.g., `js`, `py`, `rust`) |
| `output_mode` | string | No | `content`, `files_with_matches`, or `count`. Defaults to `files_with_matches` |
| `-A` | integer | No | Lines to show after match (content mode only) |
| `-B` | integer | No | Lines to show before match (content mode only) |
| `-C` | integer | No | Lines to show before and after match (content mode only) |
| `-i` | boolean | No | Case insensitive search |
| `-n` | boolean | No | Show line numbers (content mode). Defaults to true |
| `multiline` | boolean | No | Enable multiline matching |
| `head_limit` | integer | No | Limit output to first N entries |
| `offset` | integer | No | Skip first N entries |

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| `matches` | array | List of matches with `path`, `line_number`, `content`, or `count` |
| `total` | integer | Total number of matches/files |
| `truncated` | boolean | Whether results were truncated by head_limit |

## Setup

### Using tool CLI (Recommended)

Install from https://github.com/zerocore-ai/tool-cli

```bash
# Build the tool
tool run build /path/to/filesystem
```

```bash
# Validate the manifest
tool validate /path/to/filesystem
```

```bash
# Test reading a file
tool call /path/to/filesystem -m filesystem__read -p file_path=/path/to/file.txt
```

```bash
# Search for files
tool call /path/to/filesystem -m filesystem__glob -p pattern="**/*.rs"
```

```bash
# Search file contents
tool call /path/to/filesystem -m filesystem__grep -p pattern=TODO -p path=.
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
