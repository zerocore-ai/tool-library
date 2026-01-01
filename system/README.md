# System MCP Server

An MCP server providing system utilities for AI agents including sleep, datetime, and random number generation.

## Tools

### `system__sleep`

Pause execution for a specified duration.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `duration_ms` | integer | Yes | Duration to sleep in milliseconds (0 to 300000) |

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| `slept_ms` | integer | Actual duration slept in milliseconds |

### `system__get_datetime`

Get the current UTC date and time.

**Input:** None

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| `iso8601` | string | UTC timestamp in ISO 8601 format |
| `unix_ms` | integer | Unix timestamp in milliseconds |

### `system__get_random_integer`

Generate a random integer within an inclusive range.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `min` | integer | Yes | Minimum value (inclusive) |
| `max` | integer | Yes | Maximum value (inclusive) |

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| `value` | integer | Random integer in range [min, max] |

## Setup

### Using rad CLI (Recommended)

```bash
# Build the tool
rad tool run build /path/to/system

# Validate the manifest
rad tool validate /path/to/system

# Test getting current time
rad tool call /path/to/system -m system__get_datetime

# Generate a random number
rad tool call /path/to/system -m system__get_random_integer -p min=1 -p max=100

# Sleep for 1 second
rad tool call /path/to/system -m system__sleep -p duration_ms=1000
```

### Manual Build

```bash
cargo build --release
```

## Testing

```bash
cargo test
```

## License

MIT
