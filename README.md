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

## License

See [LICENSE](LICENSE) for details.
