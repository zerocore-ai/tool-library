# Linear MCP Server

A remote MCP server for interacting with Linear issues, projects, documents, comments, cycles, and milestones.

## Tools

### `list_issues`

List issues in the user's Linear workspace. Use "me" as assignee for your issues.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `limit` | number | No | Max results (default: 50, max: 250) |
| `cursor` | string | No | Next page cursor |
| `orderBy` | string | No | Sort: "createdAt" or "updatedAt" |
| `query` | string | No | Search issue title or description |
| `team` | string | No | Team name or ID |
| `state` | string | No | State type, name, or ID |
| `cycle` | string | No | Cycle name, number, or ID |
| `label` | string | No | Label name or ID |
| `assignee` | string | No | User ID, name, email, or "me" |
| `project` | string | No | Project name or ID |
| `priority` | number | No | 0=None, 1=Urgent, 2=High, 3=Normal, 4=Low |

### `get_issue`

Retrieve detailed information about an issue by ID, including attachments and git branch name.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Issue ID or identifier (e.g., LIN-123) |
| `includeRelations` | boolean | No | Include blocking/related/duplicate relations |

### `create_issue`

Create a new Linear issue.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `title` | string | Yes | Issue title |
| `team` | string | Yes | Team name or ID |
| `description` | string | No | Content as Markdown |
| `cycle` | string | No | Cycle name, number, or ID |
| `milestone` | string | No | Milestone name or ID |
| `priority` | number | No | 0=None, 1=Urgent, 2=High, 3=Normal, 4=Low |
| `project` | string | No | Project name or ID |
| `state` | string | No | State type, name, or ID |
| `assignee` | string | No | User ID, name, email, or "me" |
| `labels` | array | No | Label names or IDs |
| `dueDate` | string | No | Due date (ISO format) |
| `estimate` | number | No | Issue estimate value |

### `update_issue`

Update an existing Linear issue.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Issue ID |
| `title` | string | No | Issue title |
| `description` | string | No | Content as Markdown |
| `state` | string | No | State type, name, or ID |
| `assignee` | string | No | User ID, name, email, or "me" (null to remove) |
| `priority` | number | No | 0=None, 1=Urgent, 2=High, 3=Normal, 4=Low |
| `labels` | array | No | Label names or IDs |
| `dueDate` | string | No | Due date (ISO format) |

### `list_comments`

List comments for a specific Linear issue.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `issueId` | string | Yes | Issue ID |

### `create_comment`

Create a comment on a specific Linear issue.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `issueId` | string | Yes | Issue ID |
| `body` | string | Yes | Content as Markdown |
| `parentId` | string | No | Parent comment ID (for replies) |

### `list_projects`

List projects in the user's Linear workspace.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `limit` | number | No | Max results (default: 50, max: 250) |
| `cursor` | string | No | Next page cursor |
| `query` | string | No | Search project name |
| `state` | string | No | State type, name, or ID |
| `team` | string | No | Team name or ID |
| `member` | string | No | User ID, name, email, or "me" |
| `includeMilestones` | boolean | No | Include milestones |

### `get_project`

Retrieve details of a specific project in Linear.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `query` | string | Yes | Project ID or name |
| `includeMilestones` | boolean | No | Include milestones |
| `includeResources` | boolean | No | Include resources (documents, links) |

### `create_project`

Create a new project in Linear.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Project name |
| `team` | string | Yes | Team name or ID |
| `description` | string | No | Content as Markdown |
| `summary` | string | No | Short summary (max 255 chars) |
| `startDate` | string | No | Start date (ISO format) |
| `targetDate` | string | No | Target date (ISO format) |
| `priority` | integer | No | 0=None, 1=Urgent, 2=High, 3=Medium, 4=Low |
| `lead` | string | No | User ID, name, email, or "me" |
| `labels` | array | No | Label names or IDs |

### `update_project`

Update an existing Linear project.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Project ID |
| `name` | string | No | Project name |
| `description` | string | No | Content as Markdown |
| `state` | string | No | Project state |
| `startDate` | string | No | Start date (ISO format) |
| `targetDate` | string | No | Target date (ISO format) |
| `priority` | integer | No | 0=None, 1=Urgent, 2=High, 3=Medium, 4=Low |

### `list_teams`

List teams in the user's Linear workspace.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `limit` | number | No | Max results (default: 50, max: 250) |
| `cursor` | string | No | Next page cursor |
| `query` | string | No | Search query |

### `get_team`

Retrieve details of a specific Linear team.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `query` | string | Yes | Team UUID, key, or name |

### `list_users`

Retrieve users in the Linear workspace.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `limit` | number | No | Max results (default: 50, max: 250) |
| `cursor` | string | No | Next page cursor |
| `query` | string | No | Filter by name or email |
| `team` | string | No | Team name or ID |

### `get_user`

Retrieve details of a specific Linear user.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `query` | string | Yes | User ID, name, email, or "me" |

### `list_cycles`

Retrieve cycles for a specific Linear team.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `teamId` | string | Yes | Team ID |
| `type` | string | No | Filter: "current", "previous", or "next" |

### `list_documents`

List documents in the user's Linear workspace.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `limit` | number | No | Max results (default: 50, max: 250) |
| `cursor` | string | No | Next page cursor |
| `query` | string | No | Search query |
| `projectId` | string | No | Filter by project ID |

### `get_document`

Retrieve a Linear document by ID or slug.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Document ID or slug |

### `create_document`

Create a new document in Linear.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `title` | string | Yes | Document title |
| `content` | string | No | Content as Markdown |
| `project` | string | No | Project name or ID |
| `issue` | string | No | Issue ID or identifier |

### `update_document`

Update an existing Linear document.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Document ID or slug |
| `title` | string | No | Document title |
| `content` | string | No | Content as Markdown |

### `list_milestones`

List all milestones in a Linear project.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `project` | string | Yes | Project name or ID |

### `create_milestone`

Create a new milestone in a Linear project.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `project` | string | Yes | Project name or ID |
| `name` | string | Yes | Milestone name |
| `description` | string | No | Milestone description |
| `targetDate` | string | No | Target completion date (ISO format) |

### `list_issue_labels`

List available issue labels in a Linear workspace or team.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `limit` | number | No | Max results (default: 50, max: 250) |
| `name` | string | No | Filter by name |
| `team` | string | No | Team name or ID |

### `list_issue_statuses`

List available issue statuses in a Linear team.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `team` | string | Yes | Team name or ID |

### `create_attachment`

Create a new attachment on a specific Linear issue.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `issue` | string | Yes | Issue ID or identifier (e.g., LIN-123) |
| `base64Content` | string | Yes | Base64-encoded file content |
| `filename` | string | Yes | Filename (e.g., "screenshot.png") |
| `contentType` | string | Yes | MIME type (e.g., "image/png") |
| `title` | string | No | Attachment title |

### `extract_images`

Extract and fetch images from markdown content.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `markdown` | string | Yes | Markdown content containing image references |

### `search_documentation`

Search Linear's documentation to learn about features and usage.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `query` | string | Yes | Search query |
| `page` | number | No | Page number (default: 0) |

## Setup

### Using tool CLI

Install the CLI from https://github.com/anthropics/tool-cli

```bash
# Install from tool.store
tool install library/linear
```

```bash
# View available tools
tool info library/linear
```

```bash
# List your issues
tool call library/linear -m list_issues -p assignee=me
```

```bash
# Get issue details
tool call library/linear -m get_issue -p id=LIN-123
```

```bash
# Create a new issue
tool call library/linear -m create_issue -p title="Fix bug" -p team="Engineering"
```

### Authentication

Linear MCP uses OAuth 2.1 authentication. On first use, you'll be prompted to authorize access through your browser.

**Connection endpoints:**
- HTTP (recommended): `https://mcp.linear.app/mcp`
- SSE: `https://mcp.linear.app/sse`

## License

Apache-2.0

## References

- https://linear.app/docs/mcp
