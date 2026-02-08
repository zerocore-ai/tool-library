use std::sync::{Arc, RwLock};

use rmcp::{
    ErrorData as McpError,
    handler::server::tool::ToolRouter,
    handler::server::wrapper::Parameters,
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, Json, ServerHandler,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

//--------------------------------------------------------------------------------------------------
// Types: Error
//--------------------------------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum TodolistError {
    #[error("Content is empty or whitespace")]
    EmptyContent,

    #[error("ActiveForm is empty or whitespace")]
    EmptyActiveForm,

    #[error("Multiple items have in_progress status (only one allowed)")]
    MultipleInProgress,

    #[error("Invalid status: {0}")]
    InvalidStatus(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl TodolistError {
    /// Get the error code for this error variant.
    pub fn code(&self) -> &'static str {
        match self {
            TodolistError::EmptyContent => "EMPTY_CONTENT",
            TodolistError::EmptyActiveForm => "EMPTY_ACTIVE_FORM",
            TodolistError::MultipleInProgress => "MULTIPLE_IN_PROGRESS",
            TodolistError::InvalidStatus(_) => "INVALID_STATUS",
            TodolistError::Internal(_) => "INTERNAL_ERROR",
        }
    }

    /// Convert to MCP error with structured data.
    pub fn to_mcp_error(&self) -> McpError {
        McpError::invalid_params(self.to_string(), Some(json!({ "code": self.code() })))
    }
}

//--------------------------------------------------------------------------------------------------
// Types: Todo Item
//--------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TodoItem {
    /// Task description in imperative form (e.g., "Fix authentication bug").
    pub content: String,

    /// Current status of the task.
    pub status: TodoStatus,

    /// Task description in present continuous form (e.g., "Fixing authentication bug").
    #[serde(rename = "activeForm")]
    pub active_form: String,
}

//--------------------------------------------------------------------------------------------------
// Types: Summary
//--------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TodoSummary {
    /// Total number of items in the list.
    pub total: usize,

    /// Number of pending items.
    pub pending: usize,

    /// Number of in-progress items.
    pub in_progress: usize,

    /// Number of completed items.
    pub completed: usize,
}

impl TodoSummary {
    fn from_todos(todos: &[TodoItem]) -> Self {
        let mut summary = TodoSummary {
            total: todos.len(),
            pending: 0,
            in_progress: 0,
            completed: 0,
        };

        for todo in todos {
            match todo.status {
                TodoStatus::Pending => summary.pending += 1,
                TodoStatus::InProgress => summary.in_progress += 1,
                TodoStatus::Completed => summary.completed += 1,
            }
        }

        summary
    }
}

//--------------------------------------------------------------------------------------------------
// Types: Get
//--------------------------------------------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetInput {}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetOutput {
    /// The current list of todos.
    pub todos: Vec<TodoItem>,

    /// Summary of todo statuses.
    pub summary: TodoSummary,
}

//--------------------------------------------------------------------------------------------------
// Types: Set
//--------------------------------------------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct SetInput {
    /// The complete list of todos to set.
    pub todos: Vec<TodoItem>,
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct SetOutput {
    /// Summary of todo statuses after the update.
    pub summary: TodoSummary,
}

//--------------------------------------------------------------------------------------------------
// Types: Session State
//--------------------------------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct SessionState {
    todos: Vec<TodoItem>,
}

impl SessionState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_todos(&self) -> &[TodoItem] {
        &self.todos
    }

    pub fn set_todos(&mut self, todos: Vec<TodoItem>) {
        self.todos = todos;
    }
}

//--------------------------------------------------------------------------------------------------
// Types: Server
//--------------------------------------------------------------------------------------------------

#[derive(Clone)]
pub struct Server {
    tool_router: ToolRouter<Self>,
    session_state: Arc<RwLock<SessionState>>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl Server {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
            session_state: Arc::new(RwLock::new(SessionState::new())),
        }
    }
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

//--------------------------------------------------------------------------------------------------
// Functions: Validation
//--------------------------------------------------------------------------------------------------

fn validate_todos(todos: &[TodoItem]) -> Result<(), TodolistError> {
    let mut in_progress_count = 0;

    for todo in todos {
        // Validate content is not empty
        if todo.content.trim().is_empty() {
            return Err(TodolistError::EmptyContent);
        }

        // Validate activeForm is not empty
        if todo.active_form.trim().is_empty() {
            return Err(TodolistError::EmptyActiveForm);
        }

        // Count in_progress items
        if todo.status == TodoStatus::InProgress {
            in_progress_count += 1;
        }
    }

    // Validate only one in_progress
    if in_progress_count > 1 {
        return Err(TodolistError::MultipleInProgress);
    }

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations: Tool Router
//--------------------------------------------------------------------------------------------------

#[tool_router]
impl Server {
    /// Gets the current state of the todo list.
    ///
    /// Returns all todos with their current status and a summary.
    #[tool(
        name = "todolist__get",
        description = "Get the current state of the todo list."
    )]
    async fn get(&self, _params: Parameters<GetInput>) -> Result<Json<GetOutput>, McpError> {
        let state = self.session_state.read().map_err(|e| {
            TodolistError::Internal(format!("Failed to read state: {}", e)).to_mcp_error()
        })?;

        let todos = state.get_todos().to_vec();
        let summary = TodoSummary::from_todos(&todos);

        Ok(Json(GetOutput { todos, summary }))
    }

    /// Replaces the entire todo list.
    ///
    /// Server validates all constraints:
    /// - Content and activeForm must not be empty
    /// - Only one item can have in_progress status
    #[tool(
        name = "todolist__set",
        description = "Replace the entire todo list."
    )]
    async fn set(&self, params: Parameters<SetInput>) -> Result<Json<SetOutput>, McpError> {
        let input: SetInput = params.0;

        // Validate the todos
        validate_todos(&input.todos).map_err(|e| e.to_mcp_error())?;

        // Update the state
        let mut state = self.session_state.write().map_err(|e| {
            TodolistError::Internal(format!("Failed to write state: {}", e)).to_mcp_error()
        })?;

        state.set_todos(input.todos.clone());

        let summary = TodoSummary::from_todos(&input.todos);

        Ok(Json(SetOutput { summary }))
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations: Server Handler
//--------------------------------------------------------------------------------------------------

#[tool_handler]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: None,
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_todo_summary_empty() {
        let todos: Vec<TodoItem> = vec![];
        let summary = TodoSummary::from_todos(&todos);

        assert_eq!(summary.total, 0);
        assert_eq!(summary.pending, 0);
        assert_eq!(summary.in_progress, 0);
        assert_eq!(summary.completed, 0);
    }

    #[test]
    fn test_todo_summary_mixed() {
        let todos = vec![
            TodoItem {
                content: "Task 1".to_string(),
                status: TodoStatus::Completed,
                active_form: "Doing task 1".to_string(),
            },
            TodoItem {
                content: "Task 2".to_string(),
                status: TodoStatus::InProgress,
                active_form: "Doing task 2".to_string(),
            },
            TodoItem {
                content: "Task 3".to_string(),
                status: TodoStatus::Pending,
                active_form: "Doing task 3".to_string(),
            },
            TodoItem {
                content: "Task 4".to_string(),
                status: TodoStatus::Pending,
                active_form: "Doing task 4".to_string(),
            },
        ];
        let summary = TodoSummary::from_todos(&todos);

        assert_eq!(summary.total, 4);
        assert_eq!(summary.pending, 2);
        assert_eq!(summary.in_progress, 1);
        assert_eq!(summary.completed, 1);
    }

    #[test]
    fn test_validate_todos_valid() {
        let todos = vec![
            TodoItem {
                content: "Task 1".to_string(),
                status: TodoStatus::Pending,
                active_form: "Doing task 1".to_string(),
            },
            TodoItem {
                content: "Task 2".to_string(),
                status: TodoStatus::InProgress,
                active_form: "Doing task 2".to_string(),
            },
        ];

        assert!(validate_todos(&todos).is_ok());
    }

    #[test]
    fn test_validate_todos_empty_content() {
        let todos = vec![TodoItem {
            content: "   ".to_string(),
            status: TodoStatus::Pending,
            active_form: "Doing task".to_string(),
        }];

        let result = validate_todos(&todos);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TodolistError::EmptyContent));
    }

    #[test]
    fn test_validate_todos_empty_active_form() {
        let todos = vec![TodoItem {
            content: "Task".to_string(),
            status: TodoStatus::Pending,
            active_form: "".to_string(),
        }];

        let result = validate_todos(&todos);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TodolistError::EmptyActiveForm));
    }

    #[test]
    fn test_validate_todos_multiple_in_progress() {
        let todos = vec![
            TodoItem {
                content: "Task 1".to_string(),
                status: TodoStatus::InProgress,
                active_form: "Doing task 1".to_string(),
            },
            TodoItem {
                content: "Task 2".to_string(),
                status: TodoStatus::InProgress,
                active_form: "Doing task 2".to_string(),
            },
        ];

        let result = validate_todos(&todos);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TodolistError::MultipleInProgress
        ));
    }

    #[test]
    fn test_session_state_get_set() {
        let mut state = SessionState::new();
        assert!(state.get_todos().is_empty());

        let todos = vec![TodoItem {
            content: "Task".to_string(),
            status: TodoStatus::Pending,
            active_form: "Doing task".to_string(),
        }];

        state.set_todos(todos.clone());
        assert_eq!(state.get_todos().len(), 1);
        assert_eq!(state.get_todos()[0].content, "Task");
    }

    #[test]
    fn test_server_new() {
        let server = Server::new();
        let state = server.session_state.read().unwrap();
        assert!(state.get_todos().is_empty());
    }

    #[test]
    fn test_todo_status_serialization() {
        let pending = TodoStatus::Pending;
        let in_progress = TodoStatus::InProgress;
        let completed = TodoStatus::Completed;

        assert_eq!(serde_json::to_string(&pending).unwrap(), "\"pending\"");
        assert_eq!(
            serde_json::to_string(&in_progress).unwrap(),
            "\"in_progress\""
        );
        assert_eq!(serde_json::to_string(&completed).unwrap(), "\"completed\"");
    }

    #[test]
    fn test_todo_item_serialization() {
        let item = TodoItem {
            content: "Fix bug".to_string(),
            status: TodoStatus::InProgress,
            active_form: "Fixing bug".to_string(),
        };

        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("\"content\":\"Fix bug\""));
        assert!(json.contains("\"status\":\"in_progress\""));
        assert!(json.contains("\"activeForm\":\"Fixing bug\""));
    }

    #[test]
    fn test_todo_item_deserialization() {
        let json = r#"{"content":"Fix bug","status":"in_progress","activeForm":"Fixing bug"}"#;
        let item: TodoItem = serde_json::from_str(json).unwrap();

        assert_eq!(item.content, "Fix bug");
        assert_eq!(item.status, TodoStatus::InProgress);
        assert_eq!(item.active_form, "Fixing bug");
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(TodolistError::EmptyContent.code(), "EMPTY_CONTENT");
        assert_eq!(TodolistError::EmptyActiveForm.code(), "EMPTY_ACTIVE_FORM");
        assert_eq!(
            TodolistError::MultipleInProgress.code(),
            "MULTIPLE_IN_PROGRESS"
        );
        assert_eq!(
            TodolistError::InvalidStatus("x".to_string()).code(),
            "INVALID_STATUS"
        );
        assert_eq!(
            TodolistError::Internal("x".to_string()).code(),
            "INTERNAL_ERROR"
        );
    }

    #[test]
    fn test_mcp_error_conversion() {
        let err = TodolistError::EmptyContent;
        let mcp_err = err.to_mcp_error();
        assert_eq!(mcp_err.message, err.to_string());
        assert_eq!(
            mcp_err.data.as_ref().unwrap()["code"].as_str().unwrap(),
            "EMPTY_CONTENT"
        );
    }
}
