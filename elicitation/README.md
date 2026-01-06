# Elicitation MCP Server

An MCP server for gathering user input through structured questions with predefined options.

## Tools

### `elicitation__clarify`

Ask the user clarifying questions with predefined options.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `questions` | array | Yes | Questions to ask (1-4 questions) |

Each question object:
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `question` | string | Yes | The complete question to ask the user |
| `header` | string | Yes | Short label displayed as a tag (max 12 characters) |
| `multiSelect` | boolean | Yes | Whether multiple options can be selected |
| `options` | array | Yes | Available choices (2-4 options). An "Other" option is auto-added |

Each option object:
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `label` | string | Yes | Display text for this option (1-5 words) |
| `description` | string | Yes | Explanation of what this option means or implies |

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| `answers` | object | User's answers keyed by question index. Value is string (single-select) or array (multi-select) |
| `cancelled` | boolean | Whether the user cancelled the elicitation |

## Setup

### Using tool CLI (Recommended)

Install from https://github.com/zerocore-ai/tool-cli

```bash
# Build the tool
tool run build /path/to/elicitation
```

```bash
# Validate the manifest
tool validate /path/to/elicitation
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
