# tools

A collection of MCP tools for AI agents.

## Overview

This repository contains MCP (Model Context Protocol) servers. Each server provides a set of related tools that AI agents can use to interact with the system.

## Packages

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

## Vendor

Third-party MCP servers packaged for [tool.store](https://tool.store). These are wrappers around upstream projects with MCPB manifests.

| Server     | Description                                      |
| ---------- | ------------------------------------------------ |
| `atlassian`| Jira and Confluence integration                  |
| `elastic`  | Elasticsearch operations                         |
| `monday`   | Monday.com workspace management                  |
| `mongodb`  | MongoDB database operations                      |

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
tool info ./packages/system
tool call ./packages/bash -m bash__exec -p command="echo hello"
```

## License

See [LICENSE](LICENSE) for details.
