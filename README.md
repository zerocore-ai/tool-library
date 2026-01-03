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

## Testing

These MCP servers can be tested using [tool-cli](https://github.com/zerocore-ai/tool-cli).

### Installation

```sh
curl -fsSL https://raw.githubusercontent.com/zerocore-ai/tool-cli/main/install.sh | sh
```

### Usage

**Inspect server capabilities:**

```sh
tool info ./system          # Show tools, prompts, resources
tool info ./system --tools  # Show only tools
tool info ./system --json   # Output as JSON
```

**Call a tool method:**

```sh
# System tools
tool call ./system -m system__get_datetime
tool call ./system -m system__sleep -p duration_ms=1000
tool call ./system -m system__get_random_integer -p min=1 -p max=100

# Filesystem tools
tool call ./filesystem -m filesystem__glob -p pattern="**/*.rs"
tool call ./filesystem -m filesystem__read -p path="./README.md"

# Bash tool
tool call ./bash -m bash__exec -p command="echo hello"

# Todolist tools
tool call ./todolist -m todolist__get
tool call ./todolist -m todolist__set -p todos='[{"content":"Test task","status":"pending"}]'
```

**Verbose mode (see MCP protocol messages):**

```sh
tool call ./system -m system__get_datetime --verbose
```

**Validate manifest:**

```sh
tool validate ./system
tool validate ./system --strict  # Treat warnings as errors
```

## License

See [LICENSE](LICENSE) for details.
