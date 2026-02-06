# Elastic MCP Server

An MCP server for Elasticsearch, Kibana, and Elastic Security, connecting to Elastic's Agent Builder MCP endpoint via HTTP transport.

## Tools

### `platform_core_search`

Search and analyze data within your Elasticsearch cluster using natural language.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `query` | string | Yes | Natural language query expressing the search request |
| `index` | string | No | Index to search against (auto-selected if not provided) |

### `platform_core_execute_esql`

Execute an ES|QL query and return results in tabular format.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `query` | string | Yes | The ES|QL query to execute |

### `platform_core_generate_esql`

Generate an ES|QL query from natural language.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `query` | string | Yes | Natural language query to convert |
| `index` | string | No | Index to query against |
| `context` | string | No | Additional context for query generation |

### `platform_core_list_indices`

List indices, aliases, and datastreams from the cluster.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `pattern` | string | No | Index pattern to filter (default: "*") |

### `platform_core_index_explorer`

Find relevant indices based on a natural language query.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `query` | string | Yes | Natural language query to infer indices |
| `limit` | number | No | Max indices to return (default: 1) |
| `indexPattern` | string | No | Pattern to filter indices |

### `platform_core_get_index_mapping`

Retrieve mappings for specified indices.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `indices` | array | Yes | List of index names |

### `platform_core_get_document_by_id`

Retrieve a document by its ID and index.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Document ID |
| `index` | string | Yes | Index name |

### `platform_core_cases`

Retrieve cases from Elastic Security, Observability, or Stack Management.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `caseId` | string | No | Specific case ID to retrieve |
| `alertIds` | array | No | Alert IDs to find related cases |
| `owner` | string | No | Filter by owner: "cases", "observability", "securitySolution" |
| `start` | string | No | Start datetime (ISO format) |
| `end` | string | No | End datetime (ISO format) |
| `search` | string | No | Text search in title/description |
| `searchFields` | array | No | Fields to search: "title", "description" |
| `severity` | string/array | No | Filter: "low", "medium", "high", "critical" |
| `status` | string/array | No | Filter: "open", "closed", "in-progress" |
| `tags` | array | No | Filter by tag names |
| `assignees` | array | No | Filter by user profile UIDs |
| `reporters` | array | No | Filter by reporter usernames |
| `category` | string/array | No | Filter by category |
| `includeComments` | boolean | No | Include case comments (default: false) |

### `platform_core_product_documentation`

Search Elastic product documentation.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `query` | string | Yes | Search query |
| `product` | string | No | Filter: "kibana", "elasticsearch", "observability", "security" |
| `max` | number | No | Max documents to return (default: 3) |

### `platform_core_integration_knowledge`

Search knowledge from Fleet-installed integrations.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `query` | string | Yes | Search query about integrations |
| `max` | number | No | Max documents to return (default: 5) |

### `platform_core_get_workflow_execution_status`

Check the status of a workflow execution.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `executionId` | string | Yes | Workflow execution ID |

### `security_security_labs_search`

Search Security Labs content for malware, attack techniques, and MITRE ATT&CK information.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `query` | string | Yes | Search query for Security Labs articles |

## Setup

### Using tool CLI

Install the CLI from https://github.com/zerocore-ai/tool-cli

```bash
# Install from tool.store
tool install library/elastic
```

```bash
# View available tools
tool info library/elastic
```

```bash
# List indices
tool call library/elastic -m platform_core_list_indices
```

```bash
# Search data
tool call library/elastic -m platform_core_search -p query="show me recent errors"
```

```bash
# Generate ES|QL query
tool call library/elastic -m platform_core_generate_esql -p query="count logs by severity"
```

### Configuration

| Field | Required | Description |
|-------|----------|-------------|
| `kibana_url` | Yes | Base Kibana URL (e.g., `https://kibana.example.com` or with space: `https://kibana.example.com/s/my-space`) |
| `api_key` | Yes | Kibana API key (without "ApiKey " prefix) |

### Prerequisites

- Kibana with Agent Builder MCP enabled
- API key with appropriate Kibana application privileges

## License

Apache-2.0

## References

- https://www.elastic.co/docs/explore-analyze/ai-features/agent-builder/mcp-server
