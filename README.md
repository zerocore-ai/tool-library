# tools

A collection of MCP tools for AI agents.

## Overview

This repository contains MCP (Model Context Protocol) servers. Each server provides a set of related tools that AI agents can use to interact with the system.

## Core

| Server        | Description                              | Tools                                                                |
| ------------- | ---------------------------------------- | -------------------------------------------------------------------- |
| `bash`        | Shell command execution                  | `exec`                                                               |
| `elicitation` | User input via structured questions      | `clarify`                                                            |
| `filesystem`  | File operations                          | `read`, `write`, `edit`, `glob`, `grep`                              |
| `plugins`     | Plugin registry search and resolution    | `search`, `resolve`                                                  |
| `system`      | System utilities                         | `sleep`, `get_datetime`, `get_random_integer`                        |
| `terminal`    | PTY-based terminal sessions              | `create`, `destroy`, `list`, `send`, `read`, `info`                  |
| `todolist`    | Session-scoped task tracking             | `get`, `set`                                                         |
| `web`         | Web fetch and search                     | `fetch`, `search`                                                    |

## External

Third-party MCP servers packaged for [tool.store](https://tool.store). These are wrappers around upstream projects with MCPB manifests.

| Server     | Description                                      |
| ---------- | ------------------------------------------------ |
| `atlassian`| Jira and Confluence integration                  |
| `elastic`  | Elasticsearch operations                         |
| `monday`   | Monday.com workspace management                  |
| `mongodb`  | MongoDB database operations                      |
| `notion`   | Notion workspace content and data sources        |
| `playwright` | Browser automation and web testing             |

---

**Looking for a tool that isn't here?** [Open an issue](https://github.com/zerocore-ai/tool-library/issues/new) and let us know what you'd like to see.

## Development

```sh
make build-all    # Build all servers for all platforms
make build-bash   # Build a single server
make help         # List available targets
```

Test with [tool-cli](https://github.com/zerocore-ai/tool-cli):

```sh
tool info ./core/system
tool call ./core/bash -m exec -p command="echo hello"
```

## License

See [LICENSE](LICENSE) for details.
