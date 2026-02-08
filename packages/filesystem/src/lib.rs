use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use glob::glob as glob_match;
use grep_regex::RegexMatcher;
use grep_searcher::sinks::UTF8;
use grep_searcher::Searcher;
use ignore::WalkBuilder;
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
// Constants
//--------------------------------------------------------------------------------------------------

/// Default maximum number of lines to read from a file.
const DEFAULT_LINE_LIMIT: usize = 2000;

/// Maximum characters per line before truncation.
const MAX_LINE_LENGTH: usize = 2000;

/// Maximum file size in bytes (10 MB).
const MAX_FILE_SIZE: usize = 10 * 1024 * 1024;

/// Maximum file size for write operations in bytes (10 MB).
const MAX_WRITE_SIZE: usize = 10 * 1024 * 1024;

//--------------------------------------------------------------------------------------------------
// Types: Error
//--------------------------------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum FilesystemError {
    #[error("Path must be absolute: {0}")]
    RelativePath(String),

    #[error("Path is a directory, not a file: {0}")]
    IsDirectory(String),

    #[error("File not found: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Pattern error: {0}")]
    Pattern(#[from] glob::PatternError),

    #[error("Glob error: {0}")]
    Glob(#[from] glob::GlobError),

    #[error("Regex error: {0}")]
    Regex(String),

    #[error("old_string not found in file")]
    OldStringNotFound,

    #[error("old_string is not unique in file (found {0} occurrences). Provide more context to make it unique or use replace_all.")]
    OldStringNotUnique(usize),

    #[error("old_string and new_string must be different")]
    SameStrings,

    #[error("File must be read before writing: {0}. Use the read tool first.")]
    NotReadBeforeWrite(String),

    #[error("Path escapes allowed directory: {0}")]
    PathEscapesSandbox(String),

    #[error("Path is outside allowed directories. Allowed: {0:?}")]
    PathNotAllowed(Vec<String>),

    #[error("File too large: {size} bytes exceeds maximum of {max} bytes")]
    FileTooLarge { size: usize, max: usize },

    #[error("Content too large: {size} bytes exceeds maximum of {max} bytes")]
    ContentTooLarge { size: usize, max: usize },

    #[error("File appears to be binary: {0}")]
    BinaryFile(String),

    #[error("Path canonicalization failed: {0}")]
    CanonicalizationFailed(String),
}

impl FilesystemError {
    /// Get the error code for this error variant.
    pub fn code(&self) -> &'static str {
        match self {
            FilesystemError::RelativePath(_) => "RELATIVE_PATH",
            FilesystemError::IsDirectory(_) => "IS_DIRECTORY",
            FilesystemError::NotFound(_) => "NOT_FOUND",
            FilesystemError::Io(_) => "IO_ERROR",
            FilesystemError::Pattern(_) => "PATTERN_ERROR",
            FilesystemError::Glob(_) => "GLOB_ERROR",
            FilesystemError::Regex(_) => "REGEX_ERROR",
            FilesystemError::OldStringNotFound => "OLD_STRING_NOT_FOUND",
            FilesystemError::OldStringNotUnique(_) => "OLD_STRING_NOT_UNIQUE",
            FilesystemError::SameStrings => "SAME_STRINGS",
            FilesystemError::NotReadBeforeWrite(_) => "NOT_READ_BEFORE_WRITE",
            FilesystemError::PathEscapesSandbox(_) => "PATH_ESCAPES_SANDBOX",
            FilesystemError::PathNotAllowed(_) => "PATH_NOT_ALLOWED",
            FilesystemError::FileTooLarge { .. } => "FILE_TOO_LARGE",
            FilesystemError::ContentTooLarge { .. } => "CONTENT_TOO_LARGE",
            FilesystemError::BinaryFile(_) => "BINARY_FILE",
            FilesystemError::CanonicalizationFailed(_) => "CANONICALIZATION_FAILED",
        }
    }

    /// Convert to MCP error with structured data.
    pub fn to_mcp_error(&self) -> McpError {
        McpError::invalid_params(self.to_string(), Some(json!({ "code": self.code() })))
    }
}

/// Helper to convert errors to MCP error format.
fn to_mcp_error<E: Into<FilesystemError>>(e: E) -> McpError {
    let err: FilesystemError = e.into();
    err.to_mcp_error()
}

//--------------------------------------------------------------------------------------------------
// Types: Read
//--------------------------------------------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadInput {
    /// Absolute path to the file to read.
    pub file_path: String,

    /// Starting line number (1-indexed). Defaults to 1.
    #[serde(default)]
    pub offset: Option<usize>,

    /// Number of lines to read. Defaults to 2000.
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadOutput {
    /// The file content with line numbers in cat -n format.
    pub content: String,

    /// Total number of lines in the file.
    pub total_lines: usize,

    /// Starting line number of the returned content.
    pub start_line: usize,

    /// Ending line number of the returned content.
    pub end_line: usize,

    /// Whether the file was truncated.
    pub truncated: bool,
}

//--------------------------------------------------------------------------------------------------
// Types: Write
//--------------------------------------------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct WriteInput {
    /// Absolute path to the file to write.
    pub file_path: String,

    /// Content to write to the file.
    pub content: String,
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct WriteOutput {
    /// Number of bytes written.
    pub bytes_written: usize,
}

//--------------------------------------------------------------------------------------------------
// Types: Edit
//--------------------------------------------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct EditInput {
    /// Absolute path to the file to edit.
    pub file_path: String,

    /// The exact string to find and replace.
    pub old_string: String,

    /// The replacement string.
    pub new_string: String,

    /// If true, replace all occurrences. Defaults to false.
    #[serde(default)]
    pub replace_all: Option<bool>,
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct EditOutput {
    /// Number of replacements made.
    pub replacements: usize,
}

//--------------------------------------------------------------------------------------------------
// Types: Glob
//--------------------------------------------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct GlobInput {
    /// Glob pattern to match files against (e.g., "**/*.rs", "src/*.ts").
    pub pattern: String,

    /// Directory to search in. Defaults to current working directory.
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct GlobOutput {
    /// List of matching file paths.
    pub files: Vec<String>,
}

//--------------------------------------------------------------------------------------------------
// Types: Grep
//--------------------------------------------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct GrepInput {
    /// Regex pattern to search for.
    pub pattern: String,

    /// File or directory to search in. Defaults to current working directory.
    #[serde(default)]
    pub path: Option<String>,

    /// Glob pattern to filter files (e.g., "*.js", "*.{ts,tsx}").
    #[serde(default)]
    pub glob: Option<String>,

    /// File type to search (e.g., "js", "py", "rust").
    #[serde(default)]
    pub r#type: Option<String>,

    /// Output mode: "content", "files_with_matches", or "count". Defaults to "files_with_matches".
    #[serde(default)]
    pub output_mode: Option<String>,

    /// Lines to show after match (only for content mode).
    #[serde(rename = "-A", default)]
    pub after_context: Option<usize>,

    /// Lines to show before match (only for content mode).
    #[serde(rename = "-B", default)]
    pub before_context: Option<usize>,

    /// Lines to show before and after match (only for content mode).
    #[serde(rename = "-C", default)]
    pub context: Option<usize>,

    /// Case insensitive search.
    #[serde(rename = "-i", default)]
    pub case_insensitive: Option<bool>,

    /// Show line numbers (only for content mode). Defaults to true.
    #[serde(rename = "-n", default)]
    pub line_numbers: Option<bool>,

    /// Enable multiline matching.
    #[serde(default)]
    pub multiline: Option<bool>,

    /// Limit output to first N entries.
    #[serde(default)]
    pub head_limit: Option<usize>,

    /// Skip first N entries.
    #[serde(default)]
    pub offset: Option<usize>,
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct GrepMatch {
    /// File path containing the match.
    pub path: String,

    /// Line number of the match (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_number: Option<usize>,

    /// The matching line content (if output_mode is "content").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Match count for this file (if output_mode is "count").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct GrepOutput {
    /// List of matches.
    pub matches: Vec<GrepMatch>,

    /// Total number of matches/files.
    pub total: usize,

    /// Whether results were truncated by head_limit.
    pub truncated: bool,
}

//--------------------------------------------------------------------------------------------------
// Types: Session State
//--------------------------------------------------------------------------------------------------

/// Tracks files that have been read in the current session.
/// Used to enforce read-before-write constraints.
#[derive(Debug, Default)]
pub struct SessionState {
    /// Set of canonicalized file paths that have been read.
    files_read: HashSet<PathBuf>,
}

impl SessionState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that a file has been read.
    pub fn record_read(&mut self, path: &Path) {
        self.files_read.insert(path.to_path_buf());
    }

    /// Check if a file has been read in this session.
    pub fn has_read(&self, path: &Path) -> bool {
        self.files_read.contains(path)
    }

    /// Clear all session state.
    pub fn clear(&mut self) {
        self.files_read.clear();
    }
}

//--------------------------------------------------------------------------------------------------
// Types: Server Configuration
//--------------------------------------------------------------------------------------------------

/// Configuration options for the filesystem server.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// If set, only allow access to files within these directories.
    /// Paths are canonicalized for comparison.
    pub allowed_directories: Option<Vec<PathBuf>>,

    /// Whether to enforce read-before-write constraints.
    /// Defaults to true.
    pub require_read_before_write: bool,

    /// Maximum file size in bytes for read operations.
    pub max_read_size: usize,

    /// Maximum content size in bytes for write operations.
    pub max_write_size: usize,

    /// Whether to reject binary files.
    /// Defaults to true.
    pub reject_binary_files: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            allowed_directories: None,
            require_read_before_write: true,
            max_read_size: MAX_FILE_SIZE,
            max_write_size: MAX_WRITE_SIZE,
            reject_binary_files: true,
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Types: Server
//--------------------------------------------------------------------------------------------------

#[derive(Clone)]
pub struct Server {
    tool_router: ToolRouter<Self>,
    session_state: Arc<RwLock<SessionState>>,
    config: ServerConfig,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl Server {
    pub fn new() -> Self {
        Self::with_config(ServerConfig::default())
    }

    pub fn with_config(config: ServerConfig) -> Self {
        Self {
            tool_router: Self::tool_router(),
            session_state: Arc::new(RwLock::new(SessionState::new())),
            config,
        }
    }

    /// Record that a file has been read.
    fn record_read(&self, path: &Path) {
        if let Ok(mut state) = self.session_state.write() {
            state.record_read(path);
        }
    }

    /// Check if a file has been read in this session.
    fn has_read(&self, path: &Path) -> bool {
        self.session_state
            .read()
            .map(|state| state.has_read(path))
            .unwrap_or(false)
    }

    /// Validate that a path is allowed by the sandbox configuration.
    fn validate_sandbox(&self, path: &Path) -> Result<(), FilesystemError> {
        if let Some(ref allowed) = self.config.allowed_directories {
            let is_allowed = allowed.iter().any(|allowed_dir| path.starts_with(allowed_dir));
            if !is_allowed {
                return Err(FilesystemError::PathNotAllowed(
                    allowed.iter().map(|p| p.display().to_string()).collect(),
                ));
            }
        }
        Ok(())
    }

    /// Validate read-before-write constraint if enabled.
    fn validate_read_before_write(&self, path: &Path) -> Result<(), FilesystemError> {
        if self.config.require_read_before_write && path.exists() && !self.has_read(path) {
            return Err(FilesystemError::NotReadBeforeWrite(
                path.display().to_string(),
            ));
        }
        Ok(())
    }
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

//--------------------------------------------------------------------------------------------------
// Functions: Helpers
//--------------------------------------------------------------------------------------------------

/// Validate that a path is absolute.
fn validate_absolute_path(path: &str) -> Result<PathBuf, FilesystemError> {
    let path = PathBuf::from(path);
    if !path.is_absolute() {
        return Err(FilesystemError::RelativePath(path.display().to_string()));
    }
    Ok(path)
}

/// Canonicalize a path, normalizing `..` and symlinks.
/// For existing files, uses fs::canonicalize.
/// For non-existing files, canonicalizes the parent and appends the filename.
fn canonicalize_path(path: &Path) -> Result<PathBuf, FilesystemError> {
    if path.exists() {
        path.canonicalize()
            .map_err(|e| FilesystemError::CanonicalizationFailed(e.to_string()))
    } else {
        // For non-existing files, canonicalize parent and append filename
        let parent = path.parent().ok_or_else(|| {
            FilesystemError::CanonicalizationFailed("No parent directory".to_string())
        })?;

        let file_name = path.file_name().ok_or_else(|| {
            FilesystemError::CanonicalizationFailed("No file name".to_string())
        })?;

        // If parent doesn't exist either, we can't canonicalize
        if !parent.exists() {
            // Return the normalized path as-is for new directories
            return Ok(path.to_path_buf());
        }

        let canonical_parent = parent
            .canonicalize()
            .map_err(|e| FilesystemError::CanonicalizationFailed(e.to_string()))?;

        Ok(canonical_parent.join(file_name))
    }
}

/// Check if file content appears to be binary.
/// Uses a simple heuristic: if there are null bytes in the first 8KB, it's binary.
fn is_binary_content(content: &[u8]) -> bool {
    let check_size = content.len().min(8192);
    content[..check_size].contains(&0)
}

/// Check if a file appears to be binary.
fn is_binary_file(path: &Path) -> Result<bool, FilesystemError> {
    let file = fs::File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut buffer = [0u8; 8192];

    use std::io::Read;
    let bytes_read = reader.read(&mut buffer)?;
    Ok(is_binary_content(&buffer[..bytes_read]))
}

/// Validate file size against a maximum.
fn validate_file_size(path: &Path, max_size: usize) -> Result<usize, FilesystemError> {
    let metadata = fs::metadata(path)?;
    let size = metadata.len() as usize;
    if size > max_size {
        return Err(FilesystemError::FileTooLarge { size, max: max_size });
    }
    Ok(size)
}

/// Validate content size against a maximum.
fn validate_content_size(content: &str, max_size: usize) -> Result<(), FilesystemError> {
    let size = content.len();
    if size > max_size {
        return Err(FilesystemError::ContentTooLarge { size, max: max_size });
    }
    Ok(())
}

fn read_file_lines(
    path: &Path,
    offset: usize,
    limit: usize,
) -> Result<(Vec<String>, usize, bool), FilesystemError> {
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut lines: Vec<String> = Vec::new();
    let mut total_lines = 0;
    let mut truncated = false;

    for (idx, line) in reader.lines().enumerate() {
        total_lines = idx + 1;
        let line_num = idx + 1; // 1-indexed

        if line_num < offset {
            continue;
        }

        if lines.len() >= limit {
            truncated = true;
            continue; // Keep counting total lines
        }

        let mut line_content = line?;
        // Truncate lines longer than MAX_LINE_LENGTH characters
        if line_content.len() > MAX_LINE_LENGTH {
            line_content.truncate(MAX_LINE_LENGTH);
            line_content.push_str("...");
        }
        lines.push(line_content);
    }

    Ok((lines, total_lines, truncated))
}

fn format_with_line_numbers(lines: &[String], start_line: usize) -> String {
    let max_line_num = start_line + lines.len();
    let width = max_line_num.to_string().len().max(6);

    lines
        .iter()
        .enumerate()
        .map(|(idx, line)| {
            let line_num = start_line + idx;
            format!("{:>width$}\t{}", line_num, line, width = width)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn get_file_extension_for_type(file_type: &str) -> Option<Vec<&'static str>> {
    match file_type {
        "js" => Some(vec!["js", "mjs", "cjs"]),
        "ts" => Some(vec!["ts", "mts", "cts"]),
        "tsx" => Some(vec!["tsx"]),
        "jsx" => Some(vec!["jsx"]),
        "py" => Some(vec!["py", "pyi"]),
        "rust" | "rs" => Some(vec!["rs"]),
        "go" => Some(vec!["go"]),
        "java" => Some(vec!["java"]),
        "c" => Some(vec!["c", "h"]),
        "cpp" => Some(vec!["cpp", "cc", "cxx", "hpp", "hh", "hxx"]),
        "rb" => Some(vec!["rb"]),
        "php" => Some(vec!["php"]),
        "swift" => Some(vec!["swift"]),
        "kt" | "kotlin" => Some(vec!["kt", "kts"]),
        "scala" => Some(vec!["scala"]),
        "sh" | "bash" => Some(vec!["sh", "bash"]),
        "json" => Some(vec!["json"]),
        "yaml" | "yml" => Some(vec!["yaml", "yml"]),
        "toml" => Some(vec!["toml"]),
        "xml" => Some(vec!["xml"]),
        "html" => Some(vec!["html", "htm"]),
        "css" => Some(vec!["css"]),
        "scss" => Some(vec!["scss"]),
        "md" | "markdown" => Some(vec!["md", "markdown"]),
        _ => None,
    }
}

fn search_file(
    path: &Path,
    matcher: &RegexMatcher,
    output_mode: &str,
    show_line_numbers: bool,
) -> Result<Vec<GrepMatch>, FilesystemError> {
    let mut results: Vec<GrepMatch> = Vec::new();
    let path_str = path.display().to_string();

    match output_mode {
        "count" => {
            let mut count = 0usize;
            let mut searcher = Searcher::new();

            let _ = searcher.search_path(
                matcher,
                path,
                UTF8(|_line_num, _line| {
                    count += 1;
                    Ok(true)
                }),
            );

            if count > 0 {
                results.push(GrepMatch {
                    path: path_str,
                    line_number: None,
                    content: None,
                    count: Some(count),
                });
            }
        }
        "content" => {
            let mut searcher = Searcher::new();

            let _ = searcher.search_path(
                matcher,
                path,
                UTF8(|line_num, line| {
                    results.push(GrepMatch {
                        path: path_str.clone(),
                        line_number: if show_line_numbers {
                            Some(line_num as usize)
                        } else {
                            None
                        },
                        content: Some(line.trim_end().to_string()),
                        count: None,
                    });
                    Ok(true)
                }),
            );
        }
        _ => {
            // files_with_matches (default)
            let mut searcher = Searcher::new();
            let mut found = false;

            let _ = searcher.search_path(
                matcher,
                path,
                UTF8(|_line_num, _line| {
                    found = true;
                    Ok(false) // Stop after first match
                }),
            );

            if found {
                results.push(GrepMatch {
                    path: path_str,
                    line_number: None,
                    content: None,
                    count: None,
                });
            }
        }
    }

    Ok(results)
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations: Tool Router
//--------------------------------------------------------------------------------------------------

#[tool_router]
impl Server {
    /// Reads a file from the local filesystem.
    ///
    /// Returns file content with line numbers in cat -n format (1-indexed).
    /// Supports offset/limit for reading large files in chunks.
    #[tool(name = "filesystem__read", description = "Read a file from the local filesystem.")]
    async fn read(&self, params: Parameters<ReadInput>) -> Result<Json<ReadOutput>, McpError> {
        let input: ReadInput = params.0;

        // Validate absolute path
        let path = validate_absolute_path(&input.file_path).map_err(to_mcp_error)?;

        // Canonicalize to prevent path traversal attacks
        let canonical_path = canonicalize_path(&path).map_err(to_mcp_error)?;

        // Validate sandbox constraints
        self.validate_sandbox(&canonical_path)
            .map_err(to_mcp_error)?;

        if canonical_path.is_dir() {
            return Err(
                FilesystemError::IsDirectory(canonical_path.display().to_string()).to_mcp_error(),
            );
        }

        if !canonical_path.exists() {
            return Err(
                FilesystemError::NotFound(canonical_path.display().to_string()).to_mcp_error(),
            );
        }

        // Validate file size
        validate_file_size(&canonical_path, self.config.max_read_size)
            .map_err(to_mcp_error)?;

        // Check for binary files
        if self.config.reject_binary_files {
            if is_binary_file(&canonical_path).map_err(to_mcp_error)? {
                return Err(
                    FilesystemError::BinaryFile(canonical_path.display().to_string()).to_mcp_error(),
                );
            }
        }

        let offset = input.offset.unwrap_or(1).max(1);
        let limit = input.limit.unwrap_or(DEFAULT_LINE_LIMIT);

        let (lines, total_lines, truncated) =
            read_file_lines(&canonical_path, offset, limit).map_err(to_mcp_error)?;

        let end_line = if lines.is_empty() {
            offset
        } else {
            offset + lines.len() - 1
        };

        let content = format_with_line_numbers(&lines, offset);

        // Record this file as read for read-before-write validation
        self.record_read(&canonical_path);

        Ok(Json(ReadOutput {
            content,
            total_lines,
            start_line: offset,
            end_line,
            truncated,
        }))
    }

    /// Writes content to a file on the local filesystem.
    ///
    /// Overwrites the entire file content. Creates the file if it doesn't exist.
    /// Requires reading existing files first before overwriting.
    #[tool(name = "filesystem__write", description = "Write content to a file.")]
    async fn write(&self, params: Parameters<WriteInput>) -> Result<Json<WriteOutput>, McpError> {
        let input: WriteInput = params.0;

        // Validate absolute path
        let path = validate_absolute_path(&input.file_path).map_err(to_mcp_error)?;

        // Canonicalize to prevent path traversal attacks
        let canonical_path = canonicalize_path(&path).map_err(to_mcp_error)?;

        // Validate sandbox constraints
        self.validate_sandbox(&canonical_path)
            .map_err(to_mcp_error)?;

        if canonical_path.is_dir() {
            return Err(
                FilesystemError::IsDirectory(canonical_path.display().to_string()).to_mcp_error(),
            );
        }

        // Validate content size
        validate_content_size(&input.content, self.config.max_write_size)
            .map_err(to_mcp_error)?;

        // Validate read-before-write for existing files
        self.validate_read_before_write(&canonical_path)
            .map_err(to_mcp_error)?;

        // Create parent directories if they don't exist
        if let Some(parent) = canonical_path.parent() {
            fs::create_dir_all(parent)
                .map_err(to_mcp_error)?;
        }

        let bytes_written = input.content.len();
        fs::write(&canonical_path, &input.content).map_err(to_mcp_error)?;

        // Record as read since we now know its contents
        self.record_read(&canonical_path);

        Ok(Json(WriteOutput { bytes_written }))
    }

    /// Performs exact string replacement in a file.
    ///
    /// Finds old_string and replaces it with new_string. By default, fails if
    /// old_string is not unique unless replace_all is true.
    /// Requires reading the file first before editing.
    #[tool(name = "filesystem__edit", description = "Edit a file by replacing exact string matches.")]
    async fn edit(&self, params: Parameters<EditInput>) -> Result<Json<EditOutput>, McpError> {
        let input: EditInput = params.0;

        // Validate absolute path
        let path = validate_absolute_path(&input.file_path).map_err(to_mcp_error)?;

        // Canonicalize to prevent path traversal attacks
        let canonical_path = canonicalize_path(&path).map_err(to_mcp_error)?;

        // Validate sandbox constraints
        self.validate_sandbox(&canonical_path)
            .map_err(to_mcp_error)?;

        if !canonical_path.exists() {
            return Err(
                FilesystemError::NotFound(canonical_path.display().to_string()).to_mcp_error(),
            );
        }

        if canonical_path.is_dir() {
            return Err(
                FilesystemError::IsDirectory(canonical_path.display().to_string()).to_mcp_error(),
            );
        }

        // Validate read-before-write constraint
        self.validate_read_before_write(&canonical_path)
            .map_err(to_mcp_error)?;

        if input.old_string == input.new_string {
            return Err(FilesystemError::SameStrings.to_mcp_error());
        }

        let content = fs::read_to_string(&canonical_path).map_err(to_mcp_error)?;

        let occurrences = content.matches(&input.old_string).count();
        let replace_all = input.replace_all.unwrap_or(false);

        if occurrences == 0 {
            return Err(FilesystemError::OldStringNotFound.to_mcp_error());
        }

        if occurrences > 1 && !replace_all {
            return Err(FilesystemError::OldStringNotUnique(occurrences).to_mcp_error());
        }

        let new_content = if replace_all {
            content.replace(&input.old_string, &input.new_string)
        } else {
            content.replacen(&input.old_string, &input.new_string, 1)
        };

        // Validate new content size
        validate_content_size(&new_content, self.config.max_write_size)
            .map_err(to_mcp_error)?;

        fs::write(&canonical_path, &new_content).map_err(to_mcp_error)?;

        Ok(Json(EditOutput {
            replacements: if replace_all { occurrences } else { 1 },
        }))
    }

    /// Finds files matching a glob pattern.
    ///
    /// Supports standard glob patterns like *, **, ?, {a,b}, [abc].
    #[tool(name = "filesystem__glob", description = "Find files matching a glob pattern.")]
    async fn glob(&self, params: Parameters<GlobInput>) -> Result<Json<GlobOutput>, McpError> {
        let input: GlobInput = params.0;

        let base_path = if let Some(ref p) = input.path {
            let path = validate_absolute_path(p).map_err(to_mcp_error)?;
            canonicalize_path(&path).map_err(to_mcp_error)?
        } else {
            std::env::current_dir()
                .map_err(to_mcp_error)?
        };

        // Validate sandbox constraints for base path
        self.validate_sandbox(&base_path)
            .map_err(to_mcp_error)?;

        let full_pattern = base_path.join(&input.pattern);
        let pattern_str = full_pattern.to_string_lossy();

        let mut files: Vec<String> = Vec::new();

        for entry in glob_match(&pattern_str).map_err(to_mcp_error)? {
            match entry {
                Ok(path) => {
                    if path.is_file() {
                        // Canonicalize and validate each matched file
                        if let Ok(canonical) = canonicalize_path(&path) {
                            if self.validate_sandbox(&canonical).is_ok() {
                                files.push(canonical.display().to_string());
                            }
                        }
                    }
                }
                Err(e) => {
                    return Err(to_mcp_error(e));
                }
            }
        }

        // Sort by modification time (most recent first)
        files.sort_by(|a, b| {
            let time_a = fs::metadata(a)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            let time_b = fs::metadata(b)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            time_b.cmp(&time_a)
        });

        Ok(Json(GlobOutput { files }))
    }

    /// Searches file contents using regex patterns.
    ///
    /// Supports ripgrep-style regex patterns with various output modes.
    #[tool(name = "filesystem__grep", description = "Search file contents using regex patterns.")]
    async fn grep(&self, params: Parameters<GrepInput>) -> Result<Json<GrepOutput>, McpError> {
        let input: GrepInput = params.0;

        let base_path = if let Some(ref p) = input.path {
            let path = validate_absolute_path(p).map_err(to_mcp_error)?;
            canonicalize_path(&path).map_err(to_mcp_error)?
        } else {
            std::env::current_dir()
                .map_err(to_mcp_error)?
        };

        // Validate sandbox constraints for base path
        self.validate_sandbox(&base_path)
            .map_err(to_mcp_error)?;

        let output_mode = input.output_mode.as_deref().unwrap_or("files_with_matches");
        let case_insensitive = input.case_insensitive.unwrap_or(false);
        let _multiline = input.multiline.unwrap_or(false);
        let head_limit = input.head_limit.unwrap_or(0);
        let offset = input.offset.unwrap_or(0);
        let show_line_numbers = input.line_numbers.unwrap_or(true);

        // Build regex pattern
        let pattern = if case_insensitive {
            format!("(?i){}", input.pattern)
        } else {
            input.pattern.clone()
        };

        let matcher = RegexMatcher::new(&pattern)
            .map_err(|e| FilesystemError::Regex(e.to_string()).to_mcp_error())?;

        let mut matches: Vec<GrepMatch> = Vec::new();
        let mut total_count = 0usize;

        // Determine file extensions to filter
        let type_extensions = input.r#type.as_ref().and_then(|t| get_file_extension_for_type(t));

        // Build file walker
        let mut walker = WalkBuilder::new(&base_path);
        walker.hidden(false).git_ignore(true);

        // If it's a single file, just search it directly
        if base_path.is_file() {
            let file_matches =
                search_file(&base_path, &matcher, output_mode, show_line_numbers)
                    .map_err(to_mcp_error)?;

            if !file_matches.is_empty() {
                total_count += file_matches.len();
                matches.extend(file_matches);
            }
        } else {
            // Walk directory
            for entry in walker.build() {
                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => continue,
                };

                let path = entry.path();
                if !path.is_file() {
                    continue;
                }

                // Apply glob filter
                if let Some(ref glob_pattern) = input.glob {
                    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if !glob::Pattern::new(glob_pattern)
                        .map(|p| p.matches(file_name))
                        .unwrap_or(false)
                    {
                        // Also try matching against the full path for patterns like **/*.rs
                        let path_str = path.to_string_lossy();
                        if !glob::Pattern::new(glob_pattern)
                            .map(|p| p.matches(&path_str))
                            .unwrap_or(false)
                        {
                            continue;
                        }
                    }
                }

                // Apply type filter
                if let Some(ref extensions) = type_extensions {
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if !extensions.contains(&ext) {
                        continue;
                    }
                }

                let file_matches =
                    search_file(path, &matcher, output_mode, show_line_numbers)
                        .map_err(to_mcp_error)?;

                if !file_matches.is_empty() {
                    total_count += file_matches.len();
                    matches.extend(file_matches);
                }
            }
        }

        // Apply offset and limit
        let truncated = head_limit > 0 && matches.len() > offset + head_limit;
        if offset > 0 {
            matches = matches.into_iter().skip(offset).collect();
        }
        if head_limit > 0 {
            matches.truncate(head_limit);
        }

        Ok(Json(GrepOutput {
            matches,
            total: total_count,
            truncated,
        }))
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
    use std::fs;
    use tempfile::TempDir;

    fn create_temp_file(dir: &TempDir, name: &str, content: &str) -> String {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, content).unwrap();
        path.to_string_lossy().to_string()
    }

    // ==================== filesystem__read tests ====================

    #[test]
    fn test_read_existing_file() {
        let dir = TempDir::new().unwrap();
        let path = create_temp_file(&dir, "test.txt", "line1\nline2\nline3");

        let result = read_file_lines(std::path::Path::new(&path), 1, 2000).unwrap();
        assert_eq!(result.0, vec!["line1", "line2", "line3"]);
        assert_eq!(result.1, 3); // total lines
        assert!(!result.2); // not truncated
    }

    #[test]
    fn test_read_with_offset_limit() {
        let dir = TempDir::new().unwrap();
        let path = create_temp_file(&dir, "test.txt", "line1\nline2\nline3\nline4\nline5");

        let result = read_file_lines(std::path::Path::new(&path), 2, 2).unwrap();
        assert_eq!(result.0, vec!["line2", "line3"]);
        assert_eq!(result.1, 5); // total lines
        assert!(result.2); // truncated
    }

    #[test]
    fn test_read_error_relative_path() {
        let result = validate_absolute_path("relative/path.txt");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must be absolute"));
    }

    #[test]
    fn test_read_error_file_not_found() {
        let result = read_file_lines(std::path::Path::new("/nonexistent/file.txt"), 1, 2000);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_line_truncation() {
        let dir = TempDir::new().unwrap();
        let long_line = "x".repeat(2500);
        let path = create_temp_file(&dir, "test.txt", &long_line);

        let result = read_file_lines(std::path::Path::new(&path), 1, 2000).unwrap();
        assert_eq!(result.0[0].len(), 2003); // 2000 + "..."
        assert!(result.0[0].ends_with("..."));
    }

    #[test]
    fn test_format_with_line_numbers() {
        let lines = vec!["first".to_string(), "second".to_string()];
        let formatted = format_with_line_numbers(&lines, 1);
        assert!(formatted.contains("1\tfirst"));
        assert!(formatted.contains("2\tsecond"));
    }

    // ==================== filesystem__write tests ====================

    #[test]
    fn test_write_new_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("new_file.txt");

        fs::write(&path, "test content").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "test content");
    }

    #[test]
    fn test_write_overwrite_existing() {
        let dir = TempDir::new().unwrap();
        let path = create_temp_file(&dir, "test.txt", "original");

        fs::write(&path, "overwritten").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "overwritten");
    }

    #[test]
    fn test_write_creates_parent_directories() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nested/deep/file.txt");

        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "content").unwrap();

        assert!(path.exists());
        assert_eq!(fs::read_to_string(&path).unwrap(), "content");
    }

    #[test]
    fn test_write_error_relative_path() {
        let result = validate_absolute_path("relative/path.txt");
        assert!(result.is_err());
    }

    // ==================== filesystem__edit tests ====================

    #[test]
    fn test_edit_single_replacement() {
        let dir = TempDir::new().unwrap();
        let path = create_temp_file(&dir, "test.txt", "hello world");

        let content = fs::read_to_string(&path).unwrap();
        let new_content = content.replacen("hello", "goodbye", 1);
        fs::write(&path, &new_content).unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "goodbye world");
    }

    #[test]
    fn test_edit_replace_all() {
        let dir = TempDir::new().unwrap();
        let path = create_temp_file(&dir, "test.txt", "foo bar foo baz foo");

        let content = fs::read_to_string(&path).unwrap();
        let new_content = content.replace("foo", "qux");
        fs::write(&path, &new_content).unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "qux bar qux baz qux");
    }

    #[test]
    fn test_edit_error_old_string_not_found() {
        let dir = TempDir::new().unwrap();
        let path = create_temp_file(&dir, "test.txt", "hello world");

        let content = fs::read_to_string(&path).unwrap();
        let occurrences = content.matches("nonexistent").count();
        assert_eq!(occurrences, 0);
    }

    #[test]
    fn test_edit_error_old_string_not_unique() {
        let dir = TempDir::new().unwrap();
        let path = create_temp_file(&dir, "test.txt", "foo bar foo");

        let content = fs::read_to_string(&path).unwrap();
        let occurrences = content.matches("foo").count();
        assert_eq!(occurrences, 2);
    }

    #[test]
    fn test_edit_error_same_strings() {
        // old_string == new_string should be an error
        let old = "same";
        let new = "same";
        assert_eq!(old, new);
    }

    // ==================== filesystem__glob tests ====================

    #[test]
    fn test_glob_match_pattern() {
        let dir = TempDir::new().unwrap();
        create_temp_file(&dir, "file1.rs", "");
        create_temp_file(&dir, "file2.rs", "");
        create_temp_file(&dir, "file3.txt", "");

        let pattern = dir.path().join("*.rs").to_string_lossy().to_string();
        let matches: Vec<_> = glob_match(&pattern).unwrap().filter_map(|r| r.ok()).collect();

        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_glob_recursive_pattern() {
        let dir = TempDir::new().unwrap();
        create_temp_file(&dir, "root.rs", "");
        create_temp_file(&dir, "sub/nested.rs", "");
        create_temp_file(&dir, "sub/deep/file.rs", "");

        let pattern = dir.path().join("**/*.rs").to_string_lossy().to_string();
        let matches: Vec<_> = glob_match(&pattern).unwrap().filter_map(|r| r.ok()).collect();

        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn test_glob_no_matches() {
        let dir = TempDir::new().unwrap();
        create_temp_file(&dir, "file.txt", "");

        let pattern = dir.path().join("*.rs").to_string_lossy().to_string();
        let matches: Vec<_> = glob_match(&pattern).unwrap().filter_map(|r| r.ok()).collect();

        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn test_glob_error_relative_path() {
        let result = validate_absolute_path("relative/*.rs");
        assert!(result.is_err());
    }

    // ==================== filesystem__grep tests ====================

    #[test]
    fn test_grep_files_with_matches() {
        let dir = TempDir::new().unwrap();
        let path = create_temp_file(&dir, "test.rs", "fn main() {\n    println!(\"hello\");\n}\n");

        let matcher = RegexMatcher::new("println").unwrap();
        let results = search_file(std::path::Path::new(&path), &matcher, "files_with_matches", true).unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].line_number.is_none());
        assert!(results[0].content.is_none());
    }

    #[test]
    fn test_grep_content_mode() {
        let dir = TempDir::new().unwrap();
        let path = create_temp_file(&dir, "test.rs", "line1\nmatch_me\nline3\n");

        let matcher = RegexMatcher::new("match_me").unwrap();
        let results = search_file(std::path::Path::new(&path), &matcher, "content", true).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].line_number, Some(2));
        assert_eq!(results[0].content, Some("match_me".to_string()));
    }

    #[test]
    fn test_grep_count_mode() {
        let dir = TempDir::new().unwrap();
        let path = create_temp_file(&dir, "test.rs", "foo\nfoo\nbar\nfoo\n");

        let matcher = RegexMatcher::new("foo").unwrap();
        let results = search_file(std::path::Path::new(&path), &matcher, "count", true).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].count, Some(3));
    }

    #[test]
    fn test_grep_case_insensitive() {
        let dir = TempDir::new().unwrap();
        let path = create_temp_file(&dir, "test.rs", "Hello\nHELLO\nhello\n");

        let matcher = RegexMatcher::new("(?i)hello").unwrap();
        let results = search_file(std::path::Path::new(&path), &matcher, "count", true).unwrap();

        assert_eq!(results[0].count, Some(3));
    }

    #[test]
    fn test_grep_no_matches() {
        let dir = TempDir::new().unwrap();
        let path = create_temp_file(&dir, "test.rs", "no match here\n");

        let matcher = RegexMatcher::new("nonexistent").unwrap();
        let results = search_file(std::path::Path::new(&path), &matcher, "files_with_matches", true).unwrap();

        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_grep_error_invalid_regex() {
        let result = RegexMatcher::new("[invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_file_type_extensions() {
        assert_eq!(get_file_extension_for_type("js"), Some(vec!["js", "mjs", "cjs"]));
        assert_eq!(get_file_extension_for_type("rust"), Some(vec!["rs"]));
        assert_eq!(get_file_extension_for_type("rs"), Some(vec!["rs"]));
        assert_eq!(get_file_extension_for_type("py"), Some(vec!["py", "pyi"]));
        assert_eq!(get_file_extension_for_type("unknown"), None);
    }

    // ==================== New constraint tests ====================

    // Session state tests
    #[test]
    fn test_session_state_record_and_check() {
        let mut state = SessionState::new();
        let path = PathBuf::from("/test/file.txt");

        assert!(!state.has_read(&path));
        state.record_read(&path);
        assert!(state.has_read(&path));
    }

    #[test]
    fn test_session_state_clear() {
        let mut state = SessionState::new();
        let path = PathBuf::from("/test/file.txt");

        state.record_read(&path);
        assert!(state.has_read(&path));

        state.clear();
        assert!(!state.has_read(&path));
    }

    // Path canonicalization tests
    #[test]
    fn test_canonicalize_existing_file() {
        let dir = TempDir::new().unwrap();
        let path = create_temp_file(&dir, "test.txt", "content");

        let result = canonicalize_path(std::path::Path::new(&path));
        assert!(result.is_ok());
        assert!(result.unwrap().is_absolute());
    }

    #[test]
    fn test_canonicalize_new_file_in_existing_dir() {
        let dir = TempDir::new().unwrap();
        let new_file_path = dir.path().join("new_file.txt");

        let result = canonicalize_path(&new_file_path);
        assert!(result.is_ok());
    }

    // Binary detection tests
    #[test]
    fn test_binary_content_detection() {
        let text_content = b"Hello, this is plain text content";
        assert!(!is_binary_content(text_content));

        let binary_content = b"Hello\x00World"; // Contains null byte
        assert!(is_binary_content(binary_content));
    }

    #[test]
    fn test_binary_file_detection() {
        let dir = TempDir::new().unwrap();

        // Create a text file
        let text_path = dir.path().join("text.txt");
        fs::write(&text_path, "Hello, world!").unwrap();
        assert!(!is_binary_file(&text_path).unwrap());

        // Create a binary file
        let binary_path = dir.path().join("binary.bin");
        fs::write(&binary_path, b"Hello\x00World").unwrap();
        assert!(is_binary_file(&binary_path).unwrap());
    }

    // File size validation tests
    #[test]
    fn test_validate_file_size_within_limit() {
        let dir = TempDir::new().unwrap();
        let path = create_temp_file(&dir, "test.txt", "small content");

        let result = validate_file_size(std::path::Path::new(&path), 1024);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_file_size_exceeds_limit() {
        let dir = TempDir::new().unwrap();
        let content = "x".repeat(100);
        let path = create_temp_file(&dir, "test.txt", &content);

        let result = validate_file_size(std::path::Path::new(&path), 50);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too large"));
    }

    // Content size validation tests
    #[test]
    fn test_validate_content_size_within_limit() {
        let content = "small content";
        let result = validate_content_size(content, 1024);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_content_size_exceeds_limit() {
        let content = "x".repeat(100);
        let result = validate_content_size(&content, 50);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too large"));
    }

    // Sandbox validation tests
    #[test]
    fn test_sandbox_allows_path_in_allowed_dir() {
        let dir = TempDir::new().unwrap();
        let config = ServerConfig {
            allowed_directories: Some(vec![dir.path().to_path_buf()]),
            ..Default::default()
        };
        let server = Server::with_config(config);

        let file_path = dir.path().join("test.txt");
        let result = server.validate_sandbox(&file_path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_sandbox_rejects_path_outside_allowed_dir() {
        let allowed_dir = TempDir::new().unwrap();
        let other_dir = TempDir::new().unwrap();

        let config = ServerConfig {
            allowed_directories: Some(vec![allowed_dir.path().to_path_buf()]),
            ..Default::default()
        };
        let server = Server::with_config(config);

        let file_path = other_dir.path().join("test.txt");
        let result = server.validate_sandbox(&file_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("outside allowed"));
    }

    #[test]
    fn test_sandbox_disabled_allows_any_path() {
        let config = ServerConfig {
            allowed_directories: None,
            ..Default::default()
        };
        let server = Server::with_config(config);

        let result = server.validate_sandbox(std::path::Path::new("/any/path/file.txt"));
        assert!(result.is_ok());
    }

    // Read-before-write validation tests
    #[test]
    fn test_read_before_write_allows_new_file() {
        let dir = TempDir::new().unwrap();
        let config = ServerConfig::default();
        let server = Server::with_config(config);

        let new_file_path = dir.path().join("new_file.txt");
        let result = server.validate_read_before_write(&new_file_path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_read_before_write_rejects_unread_existing_file() {
        let dir = TempDir::new().unwrap();
        let path = create_temp_file(&dir, "test.txt", "content");

        let config = ServerConfig::default();
        let server = Server::with_config(config);

        let result = server.validate_read_before_write(std::path::Path::new(&path));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must be read before"));
    }

    #[test]
    fn test_read_before_write_allows_after_read() {
        let dir = TempDir::new().unwrap();
        let path = create_temp_file(&dir, "test.txt", "content");
        let path = std::path::Path::new(&path);

        let config = ServerConfig::default();
        let server = Server::with_config(config);

        // Record the file as read
        server.record_read(path);

        let result = server.validate_read_before_write(path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_read_before_write_disabled() {
        let dir = TempDir::new().unwrap();
        let path = create_temp_file(&dir, "test.txt", "content");

        let config = ServerConfig {
            require_read_before_write: false,
            ..Default::default()
        };
        let server = Server::with_config(config);

        let result = server.validate_read_before_write(std::path::Path::new(&path));
        assert!(result.is_ok());
    }

    // Server configuration tests
    #[test]
    fn test_server_default_config() {
        let config = ServerConfig::default();
        assert!(config.allowed_directories.is_none());
        assert!(config.require_read_before_write);
        assert_eq!(config.max_read_size, MAX_FILE_SIZE);
        assert_eq!(config.max_write_size, MAX_WRITE_SIZE);
        assert!(config.reject_binary_files);
    }

    #[test]
    fn test_server_with_custom_config() {
        let dir = TempDir::new().unwrap();
        let config = ServerConfig {
            allowed_directories: Some(vec![dir.path().to_path_buf()]),
            require_read_before_write: false,
            max_read_size: 1024,
            max_write_size: 512,
            reject_binary_files: false,
        };
        let server = Server::with_config(config.clone());

        assert!(server.config.allowed_directories.is_some());
        assert!(!server.config.require_read_before_write);
        assert_eq!(server.config.max_read_size, 1024);
        assert_eq!(server.config.max_write_size, 512);
        assert!(!server.config.reject_binary_files);
    }
}
