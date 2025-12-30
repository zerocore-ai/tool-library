# Auth MCP Server (Test)

A test MCP server demonstrating OAuth 2.0 authorization with HTTP transport.

## Purpose

This is a test tool for validating MCPB HTTP transport with OAuth authentication. It runs as an HTTP server rather than stdio.

## Transport

This server uses HTTP transport instead of stdio:

- **Transport:** HTTP
- **Endpoint:** `http://127.0.0.1:<port>/mcp`

## Configuration

### System Config (Orchestrator-Controlled)

| Option | Type | Required | Description |
|--------|------|----------|-------------|
| `port` | port | Yes | Port the HTTP server listens on |

The port is managed by the orchestrator (e.g., Radical) and assigned automatically.

## Setup

### Using rad CLI (Recommended)

```bash
# Build the tool
rad tool run build /path/to/auth

# Validate the manifest
rad tool validate /path/to/auth

# Discover available tools (rad handles port assignment)
rad tool info /path/to/auth -t
```

### Manual Build

```bash
cargo build --release
```

## HTTP vs Stdio

Unlike stdio-based MCP servers, this server:

1. Runs as an HTTP server
2. Requires port configuration via `system_config`
3. Communicates via HTTP requests to `/mcp` endpoint
4. Can handle OAuth authorization flows

## License

MIT
