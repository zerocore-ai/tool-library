# Atlassian MCP Server

An MCP server for Atlassian Jira and Confluence, connecting to the Rovo Remote MCP Server via HTTP transport with OAuth 2.1 authentication.

## Tools

### `search`

Search Jira and Confluence using Rovo Search.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `query` | string | Yes | Search query |
| `cloudId` | string | No | Cloud ID (UUID or site URL) |
| `limit` | number | No | Max results |

### `getAccessibleAtlassianResources`

Get cloud IDs for accessible Atlassian sites.

**Input:** None required.

### `atlassianUserInfo`

Get current user information.

**Input:** None required.

### `getJiraIssue`

Get Jira issue details.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cloudId` | string | Yes | Cloud ID |
| `issueIdOrKey` | string | Yes | Issue ID or key (e.g., PROJ-123) |
| `fields` | string | No | Comma-separated field names |
| `expand` | string | No | Fields to expand |

### `searchJiraIssuesUsingJql`

Search issues with JQL query.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cloudId` | string | Yes | Cloud ID |
| `jql` | string | Yes | JQL query string |
| `fields` | string | No | Comma-separated fields |
| `expand` | string | No | Expand options |
| `maxResults` | number | No | Max results |
| `startAt` | number | No | Start index |

### `createJiraIssue`

Create a new Jira issue.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cloudId` | string | Yes | Cloud ID |
| `fields` | object | Yes | Issue fields (project, issuetype, summary, etc.) |

### `editJiraIssue`

Update a Jira issue.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cloudId` | string | Yes | Cloud ID |
| `issueIdOrKey` | string | Yes | Issue ID or key |
| `fields` | object | No | Fields to update |
| `update` | object | No | Update operations |

### `addCommentToJiraIssue`

Add a comment to an issue.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cloudId` | string | Yes | Cloud ID |
| `issueIdOrKey` | string | Yes | Issue ID or key |
| `commentBody` | string | Yes | Comment text (Markdown) |
| `commentVisibility` | object | No | Visibility (type: "group"/"role", value) |

### `addWorklogToJiraIssue`

Add worklog to an issue.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cloudId` | string | Yes | Cloud ID |
| `issueIdOrKey` | string | Yes | Issue ID or key |
| `timeSpent` | string | Yes | Time format: "2h", "30m", "4d" |
| `visibility` | object | No | Visibility settings |

### `transitionJiraIssue`

Change issue status.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cloudId` | string | Yes | Cloud ID |
| `issueIdOrKey` | string | Yes | Issue ID or key |
| `transition` | object | Yes | Transition object with `id` |
| `fields` | object | No | Fields to set |
| `update` | object | No | Update operations |

### `getTransitionsForJiraIssue`

Get available transitions for an issue.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cloudId` | string | Yes | Cloud ID |
| `issueIdOrKey` | string | Yes | Issue ID or key |

### `getVisibleJiraProjects`

List accessible Jira projects.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cloudId` | string | Yes | Cloud ID |
| `maxResults` | number | No | Max results |

### `getJiraProjectIssueTypesMetadata`

Get issue types for a project.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cloudId` | string | Yes | Cloud ID |
| `projectIdOrKey` | string | Yes | Project key or ID |

### `getJiraIssueTypeMetaWithFields`

Get field metadata for an issue type.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cloudId` | string | Yes | Cloud ID |
| `projectIdOrKey` | string | Yes | Project key or ID |
| `issuetypeId` | string | Yes | Issue type ID |

### `getJiraIssueRemoteIssueLinks`

Get remote links for an issue.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cloudId` | string | Yes | Cloud ID |
| `issueIdOrKey` | string | Yes | Issue ID or key |

### `lookupJiraAccountId`

Look up user IDs by email or display name.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cloudId` | string | Yes | Cloud ID |
| `query` | string | Yes | User query |

### `getConfluencePage`

Get a Confluence page with body content.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cloudId` | string | Yes | Cloud ID |
| `pageId` | string | Yes | Page ID |
| `contentFormat` | string | No | Format: "adf" or "markdown" |

### `searchConfluenceUsingCql`

Search Confluence with CQL.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cloudId` | string | Yes | Cloud ID |
| `cql` | string | Yes | CQL query (e.g., `title ~ "meeting" AND type = page`) |
| `limit` | number | No | Max results (default: 25, max: 250) |
| `cursor` | string | No | Pagination cursor |
| `expand` | string | No | Properties to expand |

### `createConfluencePage`

Create a new Confluence page.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cloudId` | string | Yes | Cloud ID |
| `spaceId` | string | Yes | Space ID |
| `title` | string | Yes | Page title |
| `body` | string | Yes | Page content |
| `parentId` | string | No | Parent page ID |
| `contentFormat` | string | No | Format: "adf" or "markdown" |
| `status` | string | No | Status: "current" or "draft" |

### `updateConfluencePage`

Update an existing page.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cloudId` | string | Yes | Cloud ID |
| `pageId` | string | Yes | Page ID |
| `body` | string | Yes | New content |
| `title` | string | No | New title |
| `contentFormat` | string | No | Format: "adf" or "markdown" |
| `status` | string | No | Status: "current" or "draft" |
| `versionMessage` | string | No | Version comment |

### `getConfluenceSpaces`

List Confluence spaces.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cloudId` | string | Yes | Cloud ID |
| `ids` | string | No | Space IDs |
| `keys` | string | No | Space keys |
| `type` | string | No | Space type |
| `status` | string | No | Space status |
| `limit` | number | No | Max results (default: 25, max: 250) |

### `getPagesInConfluenceSpace`

Get pages in a space.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cloudId` | string | Yes | Cloud ID |
| `spaceId` | string | Yes | Space ID |
| `limit` | number | No | Max results (default: 25, max: 250) |
| `cursor` | string | No | Pagination cursor |
| `status` | string | No | Page status filter |
| `title` | string | No | Title filter |

### `getConfluencePageDescendants`

Get child pages of a page.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cloudId` | string | Yes | Cloud ID |
| `pageId` | string | Yes | Page ID |
| `limit` | number | No | Max descendants |
| `depth` | number | No | Max depth |
| `cursor` | string | No | Pagination cursor |

### `getConfluencePageFooterComments`

Get page footer comments.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cloudId` | string | Yes | Cloud ID |
| `pageId` | string | Yes | Page ID |
| `limit` | number | No | Max comments |
| `cursor` | string | No | Pagination cursor |
| `sort` | string | No | Sort order |

### `getConfluencePageInlineComments`

Get inline comments on a page.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cloudId` | string | Yes | Cloud ID |
| `pageId` | string | Yes | Page ID |
| `limit` | number | No | Max comments |
| `cursor` | string | No | Pagination cursor |
| `resolutionStatus` | string | No | Resolution status filter |

### `createConfluenceFooterComment`

Add a footer comment to a page.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cloudId` | string | Yes | Cloud ID |
| `body` | string | Yes | Comment content (Markdown) |
| `pageId` | string | No | Page ID |
| `parentCommentId` | string | No | Parent comment for replies |

### `createConfluenceInlineComment`

Add an inline comment on specific text.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cloudId` | string | Yes | Cloud ID |
| `body` | string | Yes | Comment content (Markdown) |
| `pageId` | string | No | Page ID |
| `parentCommentId` | string | No | Parent comment for replies |
| `inlineCommentProperties` | object | No | Text selection properties |

### `fetch`

Get Jira issue or Confluence page by ARI.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | ARI (e.g., `ari:cloud:...:issue/123`) or other ID |
| `cloudId` | string | No | Cloud ID (required for non-ARI IDs) |

## Setup

### Using tool CLI

Install the CLI from https://github.com/zerocore-ai/tool-cli

```bash
# Install from tool.store
tool install library/atlassian
```

```bash
# View available tools
tool info library/atlassian
```

```bash
# Get accessible resources (to find your cloudId)
tool call library/atlassian -m getAccessibleAtlassianResources
```

```bash
# Search Jira and Confluence
tool call library/atlassian -m search -p query="open bugs in Project Alpha"
```

```bash
# Search with JQL
tool call library/atlassian -m searchJiraIssuesUsingJql -p cloudId="YOUR_CLOUD_ID" -p jql="project = ALPHA AND status != Done"
```

```bash
# Search Confluence with CQL
tool call library/atlassian -m searchConfluenceUsingCql -p cloudId="YOUR_CLOUD_ID" -p cql="type = page AND title ~ \"Q2 planning\""
```

### Prerequisites

- Atlassian Cloud site with Jira and/or Confluence
- Modern browser for OAuth 2.1 consent flow

### Authentication

This server uses OAuth 2.1 browser-based authentication. On first connect, Atlassian triggers a secure OAuth consent flow. The first successful consent "installs" the MCP app for that site.

**Common Issues:**

- **"Your site admin must authorize this app"**: A site admin needs to complete the OAuth consent first
- **"Your organization admin must authorize access..."**: An org admin must allow the domain in Rovo MCP settings
- **"You don't have permission to connect from this IP address"**: IP allowlisting may be blocking access

## License

Apache-2.0

## References

- https://support.atlassian.com/atlassian-rovo-mcp-server/docs/getting-started-with-the-atlassian-remote-mcp-server/
