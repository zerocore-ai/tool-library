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
use std::collections::HashMap;
use std::io::{self, BufRead, Write};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

const MAX_QUESTIONS: usize = 4;
const MIN_OPTIONS: usize = 2;
const MAX_OPTIONS: usize = 4;
const MAX_HEADER_CHARS: usize = 12;
const MAX_LABEL_WORDS: usize = 5;

//--------------------------------------------------------------------------------------------------
// Types: Error
//--------------------------------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum ElicitationError {
    #[error("No questions provided (minimum 1)")]
    NoQuestions,

    #[error("Too many questions: {0} (maximum {MAX_QUESTIONS})")]
    TooManyQuestions(usize),

    #[error("Question {0}: question text is empty")]
    EmptyQuestion(usize),

    #[error("Question {0}: header is empty")]
    EmptyHeader(usize),

    #[error("Question {0}: header exceeds {MAX_HEADER_CHARS} characters")]
    HeaderTooLong(usize),

    #[error("Question {0}: too few options (minimum {MIN_OPTIONS})")]
    TooFewOptions(usize),

    #[error("Question {0}: too many options (maximum {MAX_OPTIONS})")]
    TooManyOptions(usize),

    #[error("Question {0}, option {1}: label is empty")]
    EmptyLabel(usize, usize),

    #[error("Question {0}, option {1}: label exceeds {MAX_LABEL_WORDS} words")]
    LabelTooLong(usize, usize),

    #[error("Question {0}, option {1}: description is empty")]
    EmptyDescription(usize, usize),

    #[error("Question {0}: invalid selection index {1}")]
    InvalidSelection(usize, usize),

    #[error("Question {0}: multi_select is false but multiple selections provided")]
    MultipleSelectionsNotAllowed(usize),

    #[error("IO error: {0}")]
    Io(String),

    #[error("User cancelled the elicitation")]
    Cancelled,
}

impl ElicitationError {
    pub fn code(&self) -> &'static str {
        match self {
            ElicitationError::NoQuestions => "NO_QUESTIONS",
            ElicitationError::TooManyQuestions(_) => "TOO_MANY_QUESTIONS",
            ElicitationError::EmptyQuestion(_) => "EMPTY_QUESTION",
            ElicitationError::EmptyHeader(_) => "EMPTY_HEADER",
            ElicitationError::HeaderTooLong(_) => "HEADER_TOO_LONG",
            ElicitationError::TooFewOptions(_) => "TOO_FEW_OPTIONS",
            ElicitationError::TooManyOptions(_) => "TOO_MANY_OPTIONS",
            ElicitationError::EmptyLabel(_, _) => "EMPTY_LABEL",
            ElicitationError::LabelTooLong(_, _) => "LABEL_TOO_LONG",
            ElicitationError::EmptyDescription(_, _) => "EMPTY_DESCRIPTION",
            ElicitationError::InvalidSelection(_, _) => "INVALID_SELECTION",
            ElicitationError::MultipleSelectionsNotAllowed(_) => "MULTIPLE_SELECTIONS_NOT_ALLOWED",
            ElicitationError::Io(_) => "IO_ERROR",
            ElicitationError::Cancelled => "CANCELLED",
        }
    }

    pub fn to_mcp_error(&self) -> McpError {
        McpError::invalid_params(self.to_string(), Some(json!({ "code": self.code() })))
    }
}

//--------------------------------------------------------------------------------------------------
// Types: Option
//--------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct QuestionOption {
    /// Display text for this option (1-5 words).
    pub label: String,

    /// Explanation of what this option means or implies.
    pub description: String,
}

//--------------------------------------------------------------------------------------------------
// Types: Question
//--------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Question {
    /// The complete question to ask the user.
    pub question: String,

    /// Short label displayed as a tag (max 12 characters).
    pub header: String,

    /// Whether multiple options can be selected.
    #[serde(rename = "multiSelect")]
    pub multi_select: bool,

    /// Available choices (2-4 options). An "Other" option is auto-added.
    pub options: Vec<QuestionOption>,
}

//--------------------------------------------------------------------------------------------------
// Types: Answer
//--------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Answer {
    /// Single selection (when multi_select is false).
    Single(String),

    /// Multiple selections (when multi_select is true).
    Multiple(Vec<String>),
}

//--------------------------------------------------------------------------------------------------
// Types: Clarify Input/Output
//--------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ClarifyInput {
    /// Questions to ask the user (1-4 questions).
    pub questions: Vec<Question>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ClarifyOutput {
    /// User's answers keyed by question index (as string).
    pub answers: HashMap<String, Answer>,

    /// Whether the user cancelled the elicitation.
    pub cancelled: bool,
}

//--------------------------------------------------------------------------------------------------
// Types: Server
//--------------------------------------------------------------------------------------------------

#[derive(Clone)]
pub struct Server {
    tool_router: ToolRouter<Self>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl Server {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
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

fn count_words(s: &str) -> usize {
    s.split_whitespace().count()
}

fn validate_questions(questions: &[Question]) -> Result<(), ElicitationError> {
    if questions.is_empty() {
        return Err(ElicitationError::NoQuestions);
    }

    if questions.len() > MAX_QUESTIONS {
        return Err(ElicitationError::TooManyQuestions(questions.len()));
    }

    for (q_idx, question) in questions.iter().enumerate() {
        // Validate question text
        if question.question.trim().is_empty() {
            return Err(ElicitationError::EmptyQuestion(q_idx));
        }

        // Validate header
        if question.header.trim().is_empty() {
            return Err(ElicitationError::EmptyHeader(q_idx));
        }
        if question.header.chars().count() > MAX_HEADER_CHARS {
            return Err(ElicitationError::HeaderTooLong(q_idx));
        }

        // Validate options count
        if question.options.len() < MIN_OPTIONS {
            return Err(ElicitationError::TooFewOptions(q_idx));
        }
        if question.options.len() > MAX_OPTIONS {
            return Err(ElicitationError::TooManyOptions(q_idx));
        }

        // Validate each option
        for (o_idx, option) in question.options.iter().enumerate() {
            if option.label.trim().is_empty() {
                return Err(ElicitationError::EmptyLabel(q_idx, o_idx));
            }
            if count_words(&option.label) > MAX_LABEL_WORDS {
                return Err(ElicitationError::LabelTooLong(q_idx, o_idx));
            }
            if option.description.trim().is_empty() {
                return Err(ElicitationError::EmptyDescription(q_idx, o_idx));
            }
        }
    }

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Functions: Elicitation
//--------------------------------------------------------------------------------------------------

/// Elicit answers from user with injectable I/O for testability.
fn elicit_answers_with_io<R: BufRead, W: Write>(
    questions: &[Question],
    reader: &mut R,
    writer: &mut W,
) -> Result<ClarifyOutput, ElicitationError> {
    let mut answers = HashMap::new();

    for (q_idx, question) in questions.iter().enumerate() {
        // Display the question
        writeln!(writer).map_err(|e| ElicitationError::Io(e.to_string()))?;
        writeln!(writer, "[{}] {}", question.header, question.question)
            .map_err(|e| ElicitationError::Io(e.to_string()))?;
        writeln!(writer).map_err(|e| ElicitationError::Io(e.to_string()))?;

        // Display options (including auto-added "Other")
        for (o_idx, option) in question.options.iter().enumerate() {
            writeln!(writer, "  {}) {} - {}", o_idx + 1, option.label, option.description)
                .map_err(|e| ElicitationError::Io(e.to_string()))?;
        }
        let other_idx = question.options.len() + 1;
        writeln!(writer, "  {}) Other - Provide custom input", other_idx)
            .map_err(|e| ElicitationError::Io(e.to_string()))?;

        // Display cancel option
        writeln!(writer, "  0) Cancel")
            .map_err(|e| ElicitationError::Io(e.to_string()))?;

        writeln!(writer).map_err(|e| ElicitationError::Io(e.to_string()))?;

        // Prompt for selection
        if question.multi_select {
            write!(writer, "Select options (comma-separated, e.g., 1,3): ")
                .map_err(|e| ElicitationError::Io(e.to_string()))?;
        } else {
            write!(writer, "Select option: ")
                .map_err(|e| ElicitationError::Io(e.to_string()))?;
        }
        writer.flush().map_err(|e| ElicitationError::Io(e.to_string()))?;

        // Read selection
        let mut input = String::new();
        reader
            .read_line(&mut input)
            .map_err(|e| ElicitationError::Io(e.to_string()))?;

        let input = input.trim();

        // Parse selection(s)
        let selections: Vec<usize> = input
            .split(',')
            .filter_map(|s| s.trim().parse::<usize>().ok())
            .collect();

        // Check for cancel
        if selections.contains(&0) {
            return Ok(ClarifyOutput {
                answers: HashMap::new(),
                cancelled: true,
            });
        }

        // Validate selection count for non-multi-select
        if !question.multi_select && selections.len() > 1 {
            return Err(ElicitationError::MultipleSelectionsNotAllowed(q_idx));
        }

        // Process selections
        let mut selected_values: Vec<String> = Vec::new();

        for sel in &selections {
            if *sel == 0 {
                continue; // Already handled cancel above
            } else if *sel <= question.options.len() {
                // Regular option
                selected_values.push(question.options[*sel - 1].label.clone());
            } else if *sel == other_idx {
                // "Other" option - get custom input
                write!(writer, "Enter custom value: ")
                    .map_err(|e| ElicitationError::Io(e.to_string()))?;
                writer.flush().map_err(|e| ElicitationError::Io(e.to_string()))?;

                let mut custom = String::new();
                reader
                    .read_line(&mut custom)
                    .map_err(|e| ElicitationError::Io(e.to_string()))?;

                selected_values.push(custom.trim().to_string());
            } else {
                return Err(ElicitationError::InvalidSelection(q_idx, *sel));
            }
        }

        // Store answer
        let answer = if question.multi_select {
            Answer::Multiple(selected_values)
        } else {
            Answer::Single(selected_values.into_iter().next().unwrap_or_default())
        };

        answers.insert(q_idx.to_string(), answer);
    }

    Ok(ClarifyOutput {
        answers,
        cancelled: false,
    })
}

/// Elicit answers using stdin/stderr.
fn elicit_answers(questions: &[Question]) -> Result<ClarifyOutput, ElicitationError> {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut writer = io::stderr();
    elicit_answers_with_io(questions, &mut reader, &mut writer)
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations: Tool Router
//--------------------------------------------------------------------------------------------------

#[tool_router]
impl Server {
    /// Ask the user clarifying questions with predefined options.
    ///
    /// Use this to gather preferences, clarify requirements, or get decisions
    /// on implementation choices. Each question can have 2-4 options, and an
    /// "Other" option for custom input is automatically added.
    #[tool(
        name = "elicitation__clarify",
        description = "Ask the user clarifying questions with predefined options. Gathers preferences, clarifies requirements, or gets decisions on implementation choices."
    )]
    async fn clarify(&self, params: Parameters<ClarifyInput>) -> Result<Json<ClarifyOutput>, McpError> {
        let input: ClarifyInput = params.0;

        // Validate questions
        validate_questions(&input.questions).map_err(|e| e.to_mcp_error())?;

        // Elicit answers from user
        let output = elicit_answers(&input.questions).map_err(|e| e.to_mcp_error())?;

        Ok(Json(output))
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
    use std::io::Cursor;

    fn make_option(label: &str, desc: &str) -> QuestionOption {
        QuestionOption {
            label: label.to_string(),
            description: desc.to_string(),
        }
    }

    fn make_question(question: &str, header: &str, multi: bool, options: Vec<QuestionOption>) -> Question {
        Question {
            question: question.to_string(),
            header: header.to_string(),
            multi_select: multi,
            options,
        }
    }

    fn make_test_questions() -> Vec<Question> {
        vec![make_question(
            "Which auth method?",
            "Auth",
            false,
            vec![
                make_option("JWT", "Token-based"),
                make_option("OAuth", "Third-party"),
            ],
        )]
    }

    fn make_multi_select_question() -> Vec<Question> {
        vec![make_question(
            "Which features?",
            "Features",
            true,
            vec![
                make_option("Logging", "Track operations"),
                make_option("Metrics", "Performance data"),
                make_option("Caching", "Speed optimization"),
            ],
        )]
    }

    #[test]
    fn test_validate_no_questions() {
        let result = validate_questions(&[]);
        assert!(matches!(result, Err(ElicitationError::NoQuestions)));
    }

    #[test]
    fn test_validate_too_many_questions() {
        let options = vec![
            make_option("A", "Option A"),
            make_option("B", "Option B"),
        ];
        let questions: Vec<Question> = (0..5)
            .map(|i| make_question(&format!("Q{}", i), "Head", false, options.clone()))
            .collect();

        let result = validate_questions(&questions);
        assert!(matches!(result, Err(ElicitationError::TooManyQuestions(5))));
    }

    #[test]
    fn test_validate_empty_question() {
        let options = vec![
            make_option("A", "Option A"),
            make_option("B", "Option B"),
        ];
        let questions = vec![make_question("   ", "Head", false, options)];

        let result = validate_questions(&questions);
        assert!(matches!(result, Err(ElicitationError::EmptyQuestion(0))));
    }

    #[test]
    fn test_validate_empty_header() {
        let options = vec![
            make_option("A", "Option A"),
            make_option("B", "Option B"),
        ];
        let questions = vec![make_question("Question?", "", false, options)];

        let result = validate_questions(&questions);
        assert!(matches!(result, Err(ElicitationError::EmptyHeader(0))));
    }

    #[test]
    fn test_validate_header_too_long() {
        let options = vec![
            make_option("A", "Option A"),
            make_option("B", "Option B"),
        ];
        let questions = vec![make_question("Question?", "ThisHeaderIsTooLong", false, options)];

        let result = validate_questions(&questions);
        assert!(matches!(result, Err(ElicitationError::HeaderTooLong(0))));
    }

    #[test]
    fn test_validate_too_few_options() {
        let options = vec![make_option("A", "Option A")];
        let questions = vec![make_question("Question?", "Head", false, options)];

        let result = validate_questions(&questions);
        assert!(matches!(result, Err(ElicitationError::TooFewOptions(0))));
    }

    #[test]
    fn test_validate_too_many_options() {
        let options = vec![
            make_option("A", "Option A"),
            make_option("B", "Option B"),
            make_option("C", "Option C"),
            make_option("D", "Option D"),
            make_option("E", "Option E"),
        ];
        let questions = vec![make_question("Question?", "Head", false, options)];

        let result = validate_questions(&questions);
        assert!(matches!(result, Err(ElicitationError::TooManyOptions(0))));
    }

    #[test]
    fn test_validate_empty_label() {
        let options = vec![
            make_option("", "Option A"),
            make_option("B", "Option B"),
        ];
        let questions = vec![make_question("Question?", "Head", false, options)];

        let result = validate_questions(&questions);
        assert!(matches!(result, Err(ElicitationError::EmptyLabel(0, 0))));
    }

    #[test]
    fn test_validate_label_too_long() {
        let options = vec![
            make_option("This label has way too many words in it", "Desc"),
            make_option("B", "Option B"),
        ];
        let questions = vec![make_question("Question?", "Head", false, options)];

        let result = validate_questions(&questions);
        assert!(matches!(result, Err(ElicitationError::LabelTooLong(0, 0))));
    }

    #[test]
    fn test_validate_empty_description() {
        let options = vec![
            make_option("A", ""),
            make_option("B", "Option B"),
        ];
        let questions = vec![make_question("Question?", "Head", false, options)];

        let result = validate_questions(&questions);
        assert!(matches!(result, Err(ElicitationError::EmptyDescription(0, 0))));
    }

    #[test]
    fn test_validate_valid_questions() {
        let options = vec![
            make_option("JWT", "Token-based auth"),
            make_option("OAuth", "Third-party auth"),
            make_option("Session", "Server-side sessions"),
        ];
        let questions = vec![
            make_question("Which auth method?", "Auth", false, options.clone()),
            make_question("Which to enable?", "Features", true, options),
        ];

        let result = validate_questions(&questions);
        assert!(result.is_ok());
    }

    #[test]
    fn test_count_words() {
        assert_eq!(count_words("hello"), 1);
        assert_eq!(count_words("hello world"), 2);
        assert_eq!(count_words("  hello   world  "), 2);
        assert_eq!(count_words("one two three four five"), 5);
        assert_eq!(count_words(""), 0);
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(ElicitationError::NoQuestions.code(), "NO_QUESTIONS");
        assert_eq!(ElicitationError::TooManyQuestions(5).code(), "TOO_MANY_QUESTIONS");
        assert_eq!(ElicitationError::EmptyQuestion(0).code(), "EMPTY_QUESTION");
        assert_eq!(ElicitationError::EmptyHeader(0).code(), "EMPTY_HEADER");
        assert_eq!(ElicitationError::HeaderTooLong(0).code(), "HEADER_TOO_LONG");
        assert_eq!(ElicitationError::TooFewOptions(0).code(), "TOO_FEW_OPTIONS");
        assert_eq!(ElicitationError::TooManyOptions(0).code(), "TOO_MANY_OPTIONS");
        assert_eq!(ElicitationError::EmptyLabel(0, 0).code(), "EMPTY_LABEL");
        assert_eq!(ElicitationError::LabelTooLong(0, 0).code(), "LABEL_TOO_LONG");
        assert_eq!(ElicitationError::EmptyDescription(0, 0).code(), "EMPTY_DESCRIPTION");
        assert_eq!(ElicitationError::InvalidSelection(0, 5).code(), "INVALID_SELECTION");
        assert_eq!(ElicitationError::MultipleSelectionsNotAllowed(0).code(), "MULTIPLE_SELECTIONS_NOT_ALLOWED");
        assert_eq!(ElicitationError::Io("test".to_string()).code(), "IO_ERROR");
        assert_eq!(ElicitationError::Cancelled.code(), "CANCELLED");
    }

    #[test]
    fn test_answer_serialization_single() {
        let answer = Answer::Single("JWT".to_string());
        let json = serde_json::to_string(&answer).unwrap();
        assert_eq!(json, "\"JWT\"");
    }

    #[test]
    fn test_answer_serialization_multiple() {
        let answer = Answer::Multiple(vec!["A".to_string(), "B".to_string()]);
        let json = serde_json::to_string(&answer).unwrap();
        assert_eq!(json, "[\"A\",\"B\"]");
    }

    #[test]
    fn test_clarify_output_serialization() {
        let mut answers = HashMap::new();
        answers.insert("0".to_string(), Answer::Single("JWT".to_string()));
        answers.insert("1".to_string(), Answer::Multiple(vec!["A".to_string(), "B".to_string()]));

        let output = ClarifyOutput {
            answers,
            cancelled: false,
        };

        let json = serde_json::to_value(&output).unwrap();
        assert_eq!(json["cancelled"], false);
        assert!(json["answers"]["0"].is_string());
        assert!(json["answers"]["1"].is_array());
    }

    #[test]
    fn test_server_new() {
        let _server = Server::new();
        // Just verify it constructs without panic
    }

    #[test]
    fn test_question_option_serialization() {
        let option = QuestionOption {
            label: "JWT".to_string(),
            description: "Token-based authentication".to_string(),
        };

        let json = serde_json::to_value(&option).unwrap();
        assert_eq!(json["label"], "JWT");
        assert_eq!(json["description"], "Token-based authentication");
    }

    #[test]
    fn test_question_serialization() {
        let question = Question {
            question: "Which auth method?".to_string(),
            header: "Auth".to_string(),
            multi_select: false,
            options: vec![
                QuestionOption {
                    label: "JWT".to_string(),
                    description: "Token-based".to_string(),
                },
            ],
        };

        let json = serde_json::to_value(&question).unwrap();
        assert_eq!(json["question"], "Which auth method?");
        assert_eq!(json["header"], "Auth");
        assert_eq!(json["multiSelect"], false);
    }

    #[test]
    fn test_clarify_input_deserialization() {
        let json = r#"{
            "questions": [{
                "question": "Which auth?",
                "header": "Auth",
                "multiSelect": true,
                "options": [
                    {"label": "JWT", "description": "Tokens"},
                    {"label": "OAuth", "description": "Third-party"}
                ]
            }]
        }"#;

        let input: ClarifyInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.questions.len(), 1);
        assert_eq!(input.questions[0].question, "Which auth?");
        assert!(input.questions[0].multi_select);
        assert_eq!(input.questions[0].options.len(), 2);
    }

    // ==================== Elicitation Tests ====================

    #[test]
    fn test_elicit_single_select_first_option() {
        let questions = make_test_questions();
        let mut reader = Cursor::new(b"1\n");
        let mut writer = Vec::new();

        let result = elicit_answers_with_io(&questions, &mut reader, &mut writer).unwrap();

        assert!(!result.cancelled);
        assert_eq!(result.answers.len(), 1);
        assert!(matches!(result.answers.get("0"), Some(Answer::Single(s)) if s == "JWT"));
    }

    #[test]
    fn test_elicit_single_select_second_option() {
        let questions = make_test_questions();
        let mut reader = Cursor::new(b"2\n");
        let mut writer = Vec::new();

        let result = elicit_answers_with_io(&questions, &mut reader, &mut writer).unwrap();

        assert!(!result.cancelled);
        assert!(matches!(result.answers.get("0"), Some(Answer::Single(s)) if s == "OAuth"));
    }

    #[test]
    fn test_elicit_cancel() {
        let questions = make_test_questions();
        let mut reader = Cursor::new(b"0\n");
        let mut writer = Vec::new();

        let result = elicit_answers_with_io(&questions, &mut reader, &mut writer).unwrap();

        assert!(result.cancelled);
        assert!(result.answers.is_empty());
    }

    #[test]
    fn test_elicit_other_option() {
        let questions = make_test_questions();
        // Option 3 is "Other" (2 options + 1)
        let mut reader = Cursor::new(b"3\nCustom Auth Method\n");
        let mut writer = Vec::new();

        let result = elicit_answers_with_io(&questions, &mut reader, &mut writer).unwrap();

        assert!(!result.cancelled);
        assert!(matches!(result.answers.get("0"), Some(Answer::Single(s)) if s == "Custom Auth Method"));
    }

    #[test]
    fn test_elicit_multi_select_single() {
        let questions = make_multi_select_question();
        let mut reader = Cursor::new(b"1\n");
        let mut writer = Vec::new();

        let result = elicit_answers_with_io(&questions, &mut reader, &mut writer).unwrap();

        assert!(!result.cancelled);
        match result.answers.get("0") {
            Some(Answer::Multiple(v)) => {
                assert_eq!(v.len(), 1);
                assert_eq!(v[0], "Logging");
            }
            _ => panic!("Expected Multiple answer"),
        }
    }

    #[test]
    fn test_elicit_multi_select_multiple() {
        let questions = make_multi_select_question();
        let mut reader = Cursor::new(b"1,3\n");
        let mut writer = Vec::new();

        let result = elicit_answers_with_io(&questions, &mut reader, &mut writer).unwrap();

        assert!(!result.cancelled);
        match result.answers.get("0") {
            Some(Answer::Multiple(v)) => {
                assert_eq!(v.len(), 2);
                assert!(v.contains(&"Logging".to_string()));
                assert!(v.contains(&"Caching".to_string()));
            }
            _ => panic!("Expected Multiple answer"),
        }
    }

    #[test]
    fn test_elicit_multi_select_all() {
        let questions = make_multi_select_question();
        let mut reader = Cursor::new(b"1,2,3\n");
        let mut writer = Vec::new();

        let result = elicit_answers_with_io(&questions, &mut reader, &mut writer).unwrap();

        assert!(!result.cancelled);
        match result.answers.get("0") {
            Some(Answer::Multiple(v)) => {
                assert_eq!(v.len(), 3);
            }
            _ => panic!("Expected Multiple answer"),
        }
    }

    #[test]
    fn test_elicit_multi_select_with_other() {
        let questions = make_multi_select_question();
        // Option 4 is "Other" (3 options + 1)
        let mut reader = Cursor::new(b"1,4\nCustom Feature\n");
        let mut writer = Vec::new();

        let result = elicit_answers_with_io(&questions, &mut reader, &mut writer).unwrap();

        assert!(!result.cancelled);
        match result.answers.get("0") {
            Some(Answer::Multiple(v)) => {
                assert_eq!(v.len(), 2);
                assert!(v.contains(&"Logging".to_string()));
                assert!(v.contains(&"Custom Feature".to_string()));
            }
            _ => panic!("Expected Multiple answer"),
        }
    }

    #[test]
    fn test_elicit_invalid_selection() {
        let questions = make_test_questions();
        // Option 5 doesn't exist (only 1, 2, 3=Other, 0=Cancel)
        let mut reader = Cursor::new(b"5\n");
        let mut writer = Vec::new();

        let result = elicit_answers_with_io(&questions, &mut reader, &mut writer);

        assert!(matches!(result, Err(ElicitationError::InvalidSelection(0, 5))));
    }

    #[test]
    fn test_elicit_multiple_not_allowed() {
        let questions = make_test_questions(); // single-select
        let mut reader = Cursor::new(b"1,2\n");
        let mut writer = Vec::new();

        let result = elicit_answers_with_io(&questions, &mut reader, &mut writer);

        assert!(matches!(result, Err(ElicitationError::MultipleSelectionsNotAllowed(0))));
    }

    #[test]
    fn test_elicit_multiple_questions() {
        let questions = vec![
            make_question(
                "Which auth?",
                "Auth",
                false,
                vec![
                    make_option("JWT", "Token-based"),
                    make_option("OAuth", "Third-party"),
                ],
            ),
            make_question(
                "Which DB?",
                "Database",
                false,
                vec![
                    make_option("Postgres", "Relational"),
                    make_option("MongoDB", "Document"),
                ],
            ),
        ];
        // Answer 1 for first question, 2 for second
        let mut reader = Cursor::new(b"1\n2\n");
        let mut writer = Vec::new();

        let result = elicit_answers_with_io(&questions, &mut reader, &mut writer).unwrap();

        assert!(!result.cancelled);
        assert_eq!(result.answers.len(), 2);
        assert!(matches!(result.answers.get("0"), Some(Answer::Single(s)) if s == "JWT"));
        assert!(matches!(result.answers.get("1"), Some(Answer::Single(s)) if s == "MongoDB"));
    }

    #[test]
    fn test_elicit_cancel_midway() {
        let questions = vec![
            make_question(
                "Q1?",
                "First",
                false,
                vec![make_option("A", "Opt A"), make_option("B", "Opt B")],
            ),
            make_question(
                "Q2?",
                "Second",
                false,
                vec![make_option("C", "Opt C"), make_option("D", "Opt D")],
            ),
        ];
        // Answer first question, cancel on second
        let mut reader = Cursor::new(b"1\n0\n");
        let mut writer = Vec::new();

        let result = elicit_answers_with_io(&questions, &mut reader, &mut writer).unwrap();

        // Should be cancelled with empty answers (cancel clears everything)
        assert!(result.cancelled);
        assert!(result.answers.is_empty());
    }

    #[test]
    fn test_elicit_output_format() {
        let questions = make_test_questions();
        let mut reader = Cursor::new(b"1\n");
        let mut writer = Vec::new();

        elicit_answers_with_io(&questions, &mut reader, &mut writer).unwrap();

        let output = String::from_utf8(writer).unwrap();
        assert!(output.contains("[Auth] Which auth method?"));
        assert!(output.contains("1) JWT - Token-based"));
        assert!(output.contains("2) OAuth - Third-party"));
        assert!(output.contains("3) Other - Provide custom input"));
        assert!(output.contains("0) Cancel"));
        assert!(output.contains("Select option:"));
    }

    #[test]
    fn test_elicit_multi_select_prompt() {
        let questions = make_multi_select_question();
        let mut reader = Cursor::new(b"1\n");
        let mut writer = Vec::new();

        elicit_answers_with_io(&questions, &mut reader, &mut writer).unwrap();

        let output = String::from_utf8(writer).unwrap();
        assert!(output.contains("Select options (comma-separated, e.g., 1,3):"));
    }

    #[test]
    fn test_elicit_whitespace_input() {
        let questions = make_test_questions();
        let mut reader = Cursor::new(b"  1  \n");
        let mut writer = Vec::new();

        let result = elicit_answers_with_io(&questions, &mut reader, &mut writer).unwrap();

        assert!(!result.cancelled);
        assert!(matches!(result.answers.get("0"), Some(Answer::Single(s)) if s == "JWT"));
    }

    #[test]
    fn test_elicit_multi_select_whitespace() {
        let questions = make_multi_select_question();
        let mut reader = Cursor::new(b" 1 , 2 , 3 \n");
        let mut writer = Vec::new();

        let result = elicit_answers_with_io(&questions, &mut reader, &mut writer).unwrap();

        match result.answers.get("0") {
            Some(Answer::Multiple(v)) => assert_eq!(v.len(), 3),
            _ => panic!("Expected Multiple answer"),
        }
    }
}
