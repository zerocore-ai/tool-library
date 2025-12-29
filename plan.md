# MCP Toolset: `todolist__*`

Session-scoped task tracking for AI agents.

---

## Overview

| Tool | Purpose |
|------|---------|
| `todolist__get` | Get current list state |
| `todolist__set` | Replace entire list (server validates constraints) |

---

## Constraints

| Constraint | Value |
|------------|-------|
| List scope | One per session |
| Max `in_progress` | 1 (server-enforced) |
| Status values | `pending`, `in_progress`, `completed` |

---

## Data Model

```json
{
  "content": "string",
  "status": "pending | in_progress | completed",
  "activeForm": "string"
}
```

- `content`: Imperative form (e.g., "Fix authentication bug")
- `activeForm`: Present continuous form (e.g., "Fixing authentication bug")
- No IDs. Items identified by array index.

---

## Status Transitions

```
pending ──────► in_progress ──────► completed
   ▲                 │
   └─────────────────┘
        (blocked)
```

- `pending` → `in_progress`: Start working
- `in_progress` → `completed`: Finish task
- `in_progress` → `pending`: Blocked/deferred
- `completed` → `pending`: Reopen
- Only ONE item can be `in_progress` at a time

---

## Tools

### `todolist__get`

Get the current state of the todo list.

**Input Schema:**

```json
{
  "type": "object",
  "additionalProperties": false,
  "properties": {}
}
```

**Output Schema:**

```json
{
  "type": "object",
  "properties": {
    "todos": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "content": { "type": "string" },
          "status": { "type": "string", "enum": ["pending", "in_progress", "completed"] },
          "activeForm": { "type": "string" }
        }
      }
    },
    "summary": {
      "type": "object",
      "properties": {
        "total": { "type": "integer" },
        "pending": { "type": "integer" },
        "in_progress": { "type": "integer" },
        "completed": { "type": "integer" }
      }
    }
  }
}
```

**Example:**

```json
// Input
{}

// Output
{
  "todos": [
    { "content": "Run build", "status": "completed", "activeForm": "Running build" },
    { "content": "Fix errors", "status": "in_progress", "activeForm": "Fixing errors" },
    { "content": "Run tests", "status": "pending", "activeForm": "Running tests" }
  ],
  "summary": { "total": 3, "pending": 1, "in_progress": 1, "completed": 1 }
}
```

---

### `todolist__set`

Replace the entire todo list. Server validates all constraints.

**Input Schema:**

```json
{
  "type": "object",
  "required": ["todos"],
  "additionalProperties": false,
  "properties": {
    "todos": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["content", "status", "activeForm"],
        "additionalProperties": false,
        "properties": {
          "content": {
            "type": "string",
            "minLength": 1
          },
          "status": {
            "type": "string",
            "enum": ["pending", "in_progress", "completed"]
          },
          "activeForm": {
            "type": "string",
            "minLength": 1
          }
        }
      }
    }
  }
}
```

**Output Schema:**

```json
{
  "type": "object",
  "properties": {
    "summary": {
      "type": "object",
      "properties": {
        "total": { "type": "integer" },
        "pending": { "type": "integer" },
        "in_progress": { "type": "integer" },
        "completed": { "type": "integer" }
      }
    }
  }
}
```

**Examples:**

*Initial planning:*

```json
// Input
{
  "todos": [
    { "content": "Run build", "status": "pending", "activeForm": "Running build" },
    { "content": "Fix errors", "status": "pending", "activeForm": "Fixing errors" },
    { "content": "Run tests", "status": "pending", "activeForm": "Running tests" }
  ]
}

// Output
{ "summary": { "total": 3, "pending": 3, "in_progress": 0, "completed": 0 } }
```

*Starting first task:*

```json
// Input
{
  "todos": [
    { "content": "Run build", "status": "in_progress", "activeForm": "Running build" },
    { "content": "Fix errors", "status": "pending", "activeForm": "Fixing errors" },
    { "content": "Run tests", "status": "pending", "activeForm": "Running tests" }
  ]
}

// Output
{ "summary": { "total": 3, "pending": 2, "in_progress": 1, "completed": 0 } }
```

*Completing and starting next:*

```json
// Input
{
  "todos": [
    { "content": "Run build", "status": "completed", "activeForm": "Running build" },
    { "content": "Fix errors", "status": "in_progress", "activeForm": "Fixing errors" },
    { "content": "Run tests", "status": "pending", "activeForm": "Running tests" }
  ]
}

// Output
{ "summary": { "total": 3, "pending": 1, "in_progress": 1, "completed": 1 } }
```

---

## Errors

```json
{
  "error": {
    "code": "string",
    "message": "string"
  }
}
```

| Code | Meaning |
|------|---------|
| `empty_content` | Content is empty or whitespace |
| `empty_active_form` | ActiveForm is empty or whitespace |
| `multiple_in_progress` | More than one item has `in_progress` status |
| `invalid_status` | Status is not one of the allowed values |

---

## Agent Workflow

### Standard Pattern

```
1. Plan         → todolist__set([{content, status: pending, activeForm}, ...])
2. Start        → todolist__set([..., {content, status: in_progress, activeForm}, ...])
3. Work         → [execute task]
4. Complete     → todolist__set([..., {content, status: completed, activeForm}, {next, status: in_progress}, ...])
5. Repeat       → steps 3-4
6. Done         → summary.pending === 0
```

---

## Example Session

```
Agent: Planning implementation...

→ todolist__set({
    "todos": [
      { "content": "Run build", "status": "in_progress", "activeForm": "Running build" },
      { "content": "Fix errors", "status": "pending", "activeForm": "Fixing errors" },
      { "content": "Run tests", "status": "pending", "activeForm": "Running tests" }
    ]
  })
← { "summary": { "total": 3, "pending": 2, "in_progress": 1, "completed": 0 } }

[Agent runs build, finds errors]

→ todolist__set({
    "todos": [
      { "content": "Run build", "status": "completed", "activeForm": "Running build" },
      { "content": "Fix errors", "status": "in_progress", "activeForm": "Fixing errors" },
      { "content": "Run tests", "status": "pending", "activeForm": "Running tests" }
    ]
  })
← { "summary": { "total": 3, "pending": 1, "in_progress": 1, "completed": 1 } }

[Agent fixes errors]

→ todolist__set({
    "todos": [
      { "content": "Run build", "status": "completed", "activeForm": "Running build" },
      { "content": "Fix errors", "status": "completed", "activeForm": "Fixing errors" },
      { "content": "Run tests", "status": "in_progress", "activeForm": "Running tests" }
    ]
  })
← { "summary": { "total": 3, "pending": 0, "in_progress": 1, "completed": 2 } }

[Agent runs tests]

→ todolist__set({
    "todos": [
      { "content": "Run build", "status": "completed", "activeForm": "Running build" },
      { "content": "Fix errors", "status": "completed", "activeForm": "Fixing errors" },
      { "content": "Run tests", "status": "completed", "activeForm": "Running tests" }
    ]
  })
← { "summary": { "total": 3, "pending": 0, "in_progress": 0, "completed": 3 } }

Agent: All tasks complete.
```

---

## Design Rationale

| Decision | Reason |
|----------|--------|
| Two tools only | Agent has list in context; no need for granular CRUD |
| No IDs | Array index is sufficient; agent manages ordering |
| Server validates on set | Constraints enforced without complex state management |
| Single `in_progress` | Structural constraint > behavioral instruction |
| `activeForm` field | Matches Claude Code's TodoWrite for UI display |
| Minimal output | Only return summary; agent already knows list contents |
| No timestamps | Not useful for agent decision-making |
| Session-scoped | Matches agent lifecycle; no persistence complexity |
