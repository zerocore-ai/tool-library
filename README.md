# tools

A collection of MCP servers for the Radical ecosystem.

## Overview

This repository contains core MCP (Model Context Protocol) servers. Each server provides a set of related tools that AI agents can use to interact with the system.

## Core Tools

| Server | Description | Tools |
|--------|-------------|-------|
| `bash` | Shell command execution | `bash__exec` |
| `filesystem` | File operations | `filesystem__read`, `filesystem__write`, `filesystem__edit`, `filesystem__glob`, `filesystem__grep` |
| `system` | System utilities | `system__sleep`, `system__get_datetime`, `system__get_random_integer` |
| `todolist` | Session-scoped task tracking | `todolist__get`, `todolist__set` |
| `web` | Web fetch and search | `web__fetch`, `web__search` |

## Design Principles

### Namespaced Tools

Tools follow the naming convention `<server>__<tool>` to prevent collisions when multiple servers are loaded together.

### Structured Output

All tools return structured JSON output with defined schemas. This enables reliable parsing and error handling by AI agents.

### Manifest-Driven

Each server includes a `manifest.json` that declares:
- Server metadata (name, version, description)
- Available tools with input/output schemas
- User configuration options
- Platform compatibility

### Error Handling

Errors are returned as structured MCP errors with error codes, enabling agents to handle different failure modes appropriately.

## Project Structure

```
tools/
├── core/
│   ├── bash/
│   ├── filesystem/
│   ├── system/
│   ├── todolist/
│   └── web/
└── test/
```

## Platform Support

Currently macOS (`darwin`) only.

## License

See [LICENSE](LICENSE) for details.
