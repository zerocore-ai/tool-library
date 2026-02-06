# monday.com MCP Server

An MCP server for interacting with the monday.com API, providing tools for boards, items, workspaces, sprints, and documents.

## Tools

### `get_user_context`

Get current user information and their relevant items (boards, folders, workspaces, dashboards).

**Input:** None required.

### `search`

Search within monday.com for boards, documents, or folders.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `searchTerm` | string | No | Search term |
| `searchType` | string | Yes | Type: "BOARD", "DOCUMENTS", or "FOLDERS" |
| `limit` | number | No | Max results (default: 100, max: 100) |
| `page` | number | No | Page number (default: 1) |
| `workspaceIds` | array | No | Workspace IDs to search within |

### `get_board_info`

Get comprehensive board information including metadata, structure, owners, and configuration.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `boardId` | number | Yes | Board ID |

### `get_board_schema`

Get board schema (columns and groups) by board ID.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `boardId` | number | Yes | Board ID |

### `get_board_items_page`

Get items from a board with pagination and optional column values.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `boardId` | number | Yes | Board ID |
| `itemIds` | array | No | Specific item IDs (max 100) |
| `searchTerm` | string | No | Search term for items |
| `limit` | number | No | Items per page (default: 25, max: 500) |
| `cursor` | string | No | Pagination cursor from previous response |
| `includeColumns` | boolean | No | Include column values (default: false) |
| `includeSubItems` | boolean | No | Include sub-items (default: false) |
| `subItemLimit` | number | No | Sub-items per item (default: 25, max: 100) |
| `filters` | array | No | Column filters to apply |
| `filtersOperator` | string | No | Filter logic: "and" or "or" (default: "and") |
| `columnIds` | array | No | Specific columns to retrieve |
| `orderBy` | array | No | Columns to order by |

### `board_insights`

Calculate board insights by filtering, grouping, and aggregating columns.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `boardId` | number | Yes | Board ID |
| `aggregations` | array | No | Aggregation functions and columns |
| `groupBy` | array | No | Columns to group by |
| `limit` | number | No | Max results (default: 1000) |
| `filters` | array | No | Filters to apply |
| `filtersOperator` | string | No | Filter logic: "and" or "or" (default: "and") |
| `orderBy` | array | No | Columns to order by |

### `get_board_activity`

Get board activity logs for a time range.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `boardId` | number | Yes | Board ID |
| `fromDate` | string | No | Start date (ISO8601, default: 30 days ago) |
| `toDate` | string | No | End date (ISO8601, default: now) |

### `list_workspaces`

List all workspaces available to the user.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `searchTerm` | string | No | Filter workspaces (alphanumeric only) |
| `limit` | number | No | Max results (default: 100, max: 100) |
| `page` | number | No | Page number (default: 1) |

### `workspace_info`

Get boards, docs, and folders in a workspace.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `workspace_id` | number | Yes | Workspace ID |

### `list_users_and_teams`

List users and/or teams with various filters.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `userIds` | array | No | Specific user IDs (max 500) |
| `teamIds` | array | No | Specific team IDs (max 500) |
| `name` | string | No | User name search (standalone) |
| `getMe` | boolean | No | Get current user (standalone) |
| `includeTeams` | boolean | No | Include all teams |
| `teamsOnly` | boolean | No | Fetch only teams |
| `includeTeamMembers` | boolean | No | Include team member details |

### `read_docs`

Get monday.com documents with content as markdown.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | string | Yes | Query type: "ids", "object_ids", or "workspace_ids" |
| `ids` | array | Yes | Array of IDs for the query type |
| `limit` | number | No | Docs per page (default: 25) |
| `order_by` | string | No | Order: "created_at" or "used_at" |
| `page` | number | No | Page number (starts at 1) |

### `get_form`

Get a monday.com form by its token.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `formToken` | string | Yes | Form token from URL (e.g., `abc123def456` from `/forms/abc123def456`) |

### `get_column_type_info`

Get information about a specific column type including JSON schema.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `columnType` | string | Yes | Column type (e.g., "text", "status", "date", "numbers") |

### `all_widgets_schema`

Fetch JSON Schema definitions for all widget types.

**Input:** None required.

### `get_monday_dev_sprints_boards`

Discover monday-dev sprints boards and their associated tasks boards.

**Input:** None required.

### `get_sprints_metadata`

Get sprint metadata from a monday-dev sprints board.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `sprintsBoardId` | number | Yes | Sprints board ID |
| `limit` | number | No | Sprints to retrieve (default: 25, max: 100) |

### `get_sprint_summary`

Get complete summary and analysis of a sprint.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `sprintId` | number | Yes | Sprint ID |

### `fetch_custom_activity`

Get custom activities from the E&A app.

**Input:** None required.

## Setup

### Using tool CLI

Install the CLI from https://github.com/zerocore-ai/tool-cli

```bash
# Install from tool.store
tool install library/monday
```

```bash
# View available tools
tool info library/monday
```

```bash
# Get current user context
tool call library/monday -m get_user_context
```

```bash
# Search for boards
tool call library/monday -m search -p searchType=BOARD -p searchTerm="marketing"
```

```bash
# Get board items
tool call library/monday -m get_board_items_page -p boardId=123456789 -p includeColumns=true
```

### Configuration

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `api_token` | Yes | - | monday.com API token |
| `read_only` | No | true | Enable read-only mode |
| `mode` | No | "api" | Tool mode: "api", "apps", or "atp" |
| `enable_dynamic_api_tools` | No | "false" | Dynamic API tools: "false", "true", or "only" |

### Prerequisites

- Node.js 20+
- monday.com API token (from Developer settings)

## License

MIT

## References

- https://github.com/mondaycom/mcp
