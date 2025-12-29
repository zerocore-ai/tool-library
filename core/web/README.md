# Web MCP Server

An MCP server providing web fetch and search capabilities for AI agents.

## Tools

### `web_fetch`

Fetches content from a URL and converts HTML to markdown.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `url` | string | Yes | URL to fetch (HTTP auto-upgrades to HTTPS) |
| `timeout_ms` | number | No | Request timeout in ms (default: 30000) |
| `max_length` | number | No | Max content bytes (default: 1MB, max: 10MB) |

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| `content` | string | Fetched content (HTML converted to markdown) |
| `final_url` | string | Final URL after redirects |
| `status` | number | HTTP status code |
| `content_type` | string | MIME type |
| `truncated` | boolean | Whether content was truncated |

### `web_search`

Searches the web using the best available provider.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `query` | string | Yes | Search query (min 2 characters) |
| `max_results` | number | No | Max results (default: 10, max: 50) |
| `allowed_domains` | string[] | No | Only include results from these domains |
| `blocked_domains` | string[] | No | Exclude results from these domains |

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| `results` | array | Search results with `title`, `url`, `snippet` |
| `count` | number | Number of results returned |
| `provider` | string | Search provider used |

## Search Providers

The server automatically selects the best available provider based on configured API keys:

| Priority | Provider | Env Variable | Free Tier |
|----------|----------|--------------|-----------|
| 1 | Brave Search | `BRAVE_SEARCH_API_KEY` | 2000/month |
| 2 | Tavily | `TAVILY_API_KEY` | 1000/month |
| 3 | SerpAPI | `SERPAPI_API_KEY` | 100/month |
| 4 | DuckDuckGo | (none) | Unreliable* |

*DuckDuckGo uses HTML scraping and may trigger bot detection. Use an API-based provider for reliable results.

## Configuration

### Environment Variables

Set one or more API keys to enable reliable search:

```bash
# Recommended - best free tier
export BRAVE_SEARCH_API_KEY="your-brave-api-key"

# Alternative providers
export TAVILY_API_KEY="your-tavily-api-key"
export SERPAPI_API_KEY="your-serpapi-api-key"
```

### MCPB User Config

When installed via MCPB, configure API keys through the manifest:

```json
{
  "user_config": {
    "brave_api_key": "your-brave-api-key",
    "tavily_api_key": "your-tavily-api-key",
    "serpapi_api_key": "your-serpapi-api-key"
  }
}
```

## Getting API Keys

| Provider | Sign Up |
|----------|---------|
| Brave Search | https://brave.com/search/api/ |
| Tavily | https://tavily.com/ |
| SerpAPI | https://serpapi.com/ |

## Building

```bash
cargo build --release
```

## Running

```bash
# Run the MCP server (stdio transport)
./target/release/web
```

## Testing

```bash
# Run unit tests
cargo test

# Run integration tests (requires network)
cargo test --test integration
```

## License

MIT
