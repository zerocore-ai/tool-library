# Hello MCP Server (Test)

A simple test MCP server demonstrating user configuration with personalized greetings.

## Purpose

This is a test tool for validating MCPB user configuration. It requires a `username` config value and uses it for personalized responses.

## Configuration

### MCPB User Config

| Option | Type | Required | Description |
|--------|------|----------|-------------|
| `username` | string | Yes | Your name for personalized greetings |

When installing via MCPB:

```json
{
  "user_config": {
    "username": "Alice"
  }
}
```

## Setup

### Using rad CLI (Recommended)

```bash
# Build the tool
rad tool run build /path/to/hello

# Validate the manifest
rad tool validate /path/to/hello

# Discover available tools
rad tool info /path/to/hello -t -c username=Alice
```

### Manual Build

```bash
cargo build --release
```

## License

MIT
