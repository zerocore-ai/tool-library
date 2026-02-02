# tools

A collection of MCP tools for AI agents.

## Overview

This repository contains MCP (Model Context Protocol) servers. Each server provides a set of related tools that AI agents can use to interact with the system.

## Tools

| Server        | Description                              | Tools                                                                                               |
| ------------- | ---------------------------------------- | --------------------------------------------------------------------------------------------------- |
| `bash`        | Shell command execution                  | `bash__exec`                                                                                        |
| `elicitation` | User input via structured questions      | `elicitation__clarify`                                                                              |
| `filesystem`  | File operations                          | `filesystem__read`, `filesystem__write`, `filesystem__edit`, `filesystem__glob`, `filesystem__grep` |
| `plugins`     | Plugin registry search and resolution    | `plugins__search`, `plugins__resolve`                                                               |
| `system`      | System utilities                         | `system__sleep`, `system__get_datetime`, `system__get_random_integer`                               |
| `terminal`    | PTY-based terminal sessions              | `terminal__create`, `terminal__destroy`, `terminal__list`, `terminal__send`, `terminal__read`, `terminal__info` |
| `todolist`    | Session-scoped task tracking             | `todolist__get`, `todolist__set`                                                                    |
| `web`         | Web fetch and search                     | `web__fetch`, `web__search`                                                                         |

## Design Principles

### Namespaced Tools

Tools follow the naming convention `<server>__<tool>` to prevent collisions when multiple servers are loaded together.

### Structured Output

All tools return structured JSON output with defined schemas. This enables reliable parsing and error handling by AI agents.

### MCPB Packaging

Each server follows the [MCPB (MCP Bundles)](https://github.com/modelcontextprotocol/mcpb) specification with a `manifest.json` that declares:

- Server metadata (name, version, description)
- Available tools with input/output schemas
- User configuration options
- Platform compatibility

### Error Handling

Errors are returned as structured MCP errors with error codes, enabling agents to handle different failure modes appropriately.

```json
{
  "code": -32602,
  "message": "Resource not found: my-plugin",
  "data": { "code": "NOT_FOUND" }
}
```

## Development

### Prerequisites

- Rust toolchain (1.92.0+)
- [cross](https://github.com/cross-rs/cross) for Linux cross-compilation
- Docker (required by cross)

```sh
# Install cross
cargo install cross --git https://github.com/cross-rs/cross
```

### Building

The Makefile supports building for multiple platforms:

| Target | Platform | Method |
|--------|----------|--------|
| `darwin-arm64` | macOS ARM64 | Native cargo |
| `linux-arm64` | Linux ARM64 | cross (Docker) |
| `linux-x86_64` | Linux x86_64 | cross (Docker) |

**Build commands:**

```sh
# Build a single server for all platforms
make build-bash

# Build a single server for a specific platform
make build-bash-darwin-arm64
make build-bash-linux-arm64
make build-bash-linux-x86_64

# Build all servers for all platforms
make build-all

# Clean dist directories
make clean

# Clean dist and cargo target directories
make clean-all
```

**Output:**

Binaries are placed in each server's `dist/` directory:

```
packages/bash/dist/
├── bash-darwin-arm64
├── bash-linux-arm64
└── bash-linux-x86_64
```

### Available Servers

```sh
make help  # Lists all available servers and targets
```

## Testing

These MCP servers can be tested using [tool-cli](https://github.com/zerocore-ai/tool-cli).

### Installation

```sh
curl -fsSL https://raw.githubusercontent.com/zerocore-ai/tool-cli/main/install.sh | sh
```

### Usage

**Inspect server capabilities:**

```sh
tool info ./packages/system          # Show tools, prompts, resources
tool info ./packages/system --tools  # Show only tools
tool info ./packages/system --json   # Output as JSON
```

**Call a tool method:**

```sh
# System tools
tool call ./packages/system -m system__get_datetime
tool call ./packages/system -m system__sleep -p duration_ms=1000
tool call ./packages/system -m system__get_random_integer -p min=1 -p max=100

# Filesystem tools
tool call ./packages/filesystem -m filesystem__glob -p pattern="**/*.rs"
tool call ./packages/filesystem -m filesystem__read -p path="./README.md"

# Bash tool
tool call ./packages/bash -m bash__exec -p command="echo hello"

# Todolist tools
tool call ./packages/todolist -m todolist__get
tool call ./packages/todolist -m todolist__set -p todos='[{"content":"Test task","status":"pending"}]'
```

**Verbose mode (see MCP protocol messages):**

```sh
tool call ./packages/system -m system__get_datetime --verbose
```

**Validate manifest:**

```sh
tool validate ./packages/system
tool validate ./packages/system --strict  # Treat warnings as errors
```

## License

See [LICENSE](LICENSE) for details.
