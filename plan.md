# Terminal MCP Server - Implementation Plan

## Overview

An MCP server that provides AI agents with full pseudo-terminal (PTY) access, enabling interaction with interactive terminal applications beyond simple command execution.

**Key Differentiator**: Unlike child process-based bash execution, this tool provides persistent PTY sessions that support TUI apps, interactive prompts, shell job control, and proper terminal emulation.

**Multi-Session Support**: Agents can create multiple independent terminal sessions, each running any program (shell by default). Sessions are identified by unique IDs and managed independently.

**Platform Support**: Unix only (macOS, Linux). Windows is not supported due to fundamental differences in ConPTY.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         MCP Server                               │
├─────────────────────────────────────────────────────────────────┤
│  Session Management Tools                                        │
│  ├── terminal__create_session   (spawn program in new PTY)      │
│  ├── terminal__destroy_session  (terminate session)             │
│  └── terminal__list_sessions    (list active sessions)          │
├─────────────────────────────────────────────────────────────────┤
│  Session Interaction Tools (all require session_id)             │
│  ├── terminal__send             (input + optional read)         │
│  ├── terminal__read             (screen/new/scrollback views)   │
│  └── terminal__get_info         (state query)                   │
├─────────────────────────────────────────────────────────────────┤
│  Core Components                                                 │
│  ├── SessionManager        (session lifecycle, lookup)          │
│  ├── TerminalSession       (wrapper with metadata)              │
│  ├── SessionReader         (background PTY reader thread)       │
│  ├── PtySession            (PTY lifecycle, I/O)                 │
│  ├── ScreenBuffer          (terminal emulation, cursor)         │
│  ├── ScrollbackBuffer      (historical output ring buffer)      │
│  ├── OutputTracker         ("new" output since last read)       │
│  ├── PromptDetector        (shell prompt pattern matching)      │
│  └── AnsiStripper          (plain text conversion)              │
├─────────────────────────────────────────────────────────────────┤
│  Sessions (0..N, created on demand)                              │
│  ├── Session "sess_abc123" → PTY → /bin/bash                    │
│  ├── Session "sess_def456" → PTY → vim file.txt                 │
│  └── Session "sess_ghi789" → PTY → htop                         │
└─────────────────────────────────────────────────────────────────┘
```

### Background Reader Architecture

Each session has a dedicated reader thread that continuously reads PTY output:

```
┌─────────────────────────────────────────────────────────────────┐
│                      TerminalSession                             │
├─────────────────────────────────────────────────────────────────┤
│  ┌──────────────────┐      ┌─────────────────────────────────┐  │
│  │  Reader Thread   │─────▶│  mpsc channel (ReaderMessage)   │  │
│  │  (blocking I/O)  │      └─────────────────────────────────┘  │
│  └──────────────────┘                    │                      │
│          ▲                               ▼                      │
│     PTY master              drain() processes into:             │
│                              ├── ScreenBuffer                   │
│                              ├── ScrollbackBuffer               │
│                              └── OutputTracker                  │
└─────────────────────────────────────────────────────────────────┘

ReaderMessage:
  ├── Data(Vec<u8>)           # Raw PTY output
  ├── Exited(Option<i32>)     # Process exited with code
  └── Error(String)           # Fatal read error
```

---

## Project Structure

```
core/terminal/
├── Cargo.toml
├── manifest.json
├── bin/
│   └── main.rs                 # MCP server entry point
└── lib/
    ├── lib.rs                  # Re-exports (Unix-only guard)
    ├── server.rs               # MCP server implementation
    ├── config.rs               # User configuration
    ├── session/
    │   ├── mod.rs
    │   ├── manager.rs          # SessionManager (HashMap of sessions)
    │   ├── session.rs          # TerminalSession (wrapper with metadata)
    │   ├── reader.rs           # SessionReader (background PTY reader)
    │   └── id.rs               # Session ID generation
    ├── pty/
    │   ├── mod.rs
    │   ├── session.rs          # PTY session management
    │   └── env.rs              # Environment variable filtering
    ├── terminal/
    │   ├── mod.rs
    │   ├── state.rs            # TerminalState (per-session state)
    │   ├── screen.rs           # Screen buffer (visible terminal)
    │   ├── scrollback.rs       # Scrollback ring buffer
    │   ├── tracker.rs          # Output tracking for "new" view
    │   ├── emulator.rs         # ANSI/VT100 sequence processing
    │   └── cursor.rs           # Cursor state
    ├── input/
    │   ├── mod.rs
    │   ├── keys.rs             # Special key encoding
    │   ├── modifiers.rs        # Ctrl/Alt/Shift handling
    │   └── paste.rs            # Bracketed paste mode
    ├── output/
    │   ├── mod.rs
    │   ├── ansi.rs             # ANSI code stripping
    │   └── formatter.rs        # Plain vs raw output
    ├── tools/
    │   ├── mod.rs
    │   ├── create_session.rs   # terminal__create_session
    │   ├── destroy_session.rs  # terminal__destroy_session
    │   ├── list_sessions.rs    # terminal__list_sessions
    │   ├── send.rs             # terminal__send
    │   ├── read.rs             # terminal__read
    │   └── info.rs             # terminal__get_info
    └── types.rs                # Shared types, errors
```

---

## Dependencies

```toml
[package]
name = "terminal"
version = "0.1.0"
edition = "2024"

[dependencies]
# MCP protocol
rmcp = { version = "0.12", features = ["server", "macros", "transport-io"] }

# PTY
portable-pty = "0.8"            # Cross-platform PTY

# Terminal emulation
vte = "0.13"                    # ANSI/VT parser

# Async runtime
tokio = { version = "1", features = ["macros", "rt-multi-thread", "io-std", "sync", "time", "signal"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
schemars = "1"

# Utilities
regex = "1"                     # Prompt pattern matching
thiserror = "2"                 # Error handling
tracing = "0.1"                 # Logging
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
uuid = { version = "1", features = ["v4"] }  # Session IDs
unicode-width = "0.2"           # Wide character handling

[dev-dependencies]
tempfile = "3"
```

---

## Tool Specifications

### Session Management Tools

#### terminal__create_session

Create a new terminal session running any program (shell by default).

```yaml
Input:
  program: string = "$SHELL"    # Program to run (shell is default)
  args: [string] = []           # Program arguments
  rows: int = 24                # Terminal height (overrides config default)
  cols: int = 80                # Terminal width (overrides config default)
  env: object = {}              # Additional environment variables
  cwd: string = null            # Working directory (null = inherit)
  wait_ready: bool = null       # Wait for prompt before returning (default: true for shells)
  ready_timeout_ms: int = 5000  # Timeout for wait_ready

Output:
  session_id: string            # Unique session identifier (e.g., "sess_a1b2c3d4")
  pid: int                      # Process ID of spawned program
  program: string               # Resolved program path
  dimensions: { rows, cols }
```

**Note**: Terminal dimensions are fixed at creation. To use different dimensions, create a new session.

**Examples**:
```yaml
# Default shell
→ terminal__create_session()
← { session_id: "sess_abc123", pid: 12345, program: "/bin/bash", ... }

# Vim session
→ terminal__create_session(program: "vim", args: ["file.txt"])
← { session_id: "sess_def456", pid: 12346, program: "/usr/bin/vim", ... }

# Python REPL with custom size
→ terminal__create_session(program: "python3", rows: 40, cols: 120)
← { session_id: "sess_ghi789", pid: 12347, program: "/usr/bin/python3", ... }

# Shell with custom environment
→ terminal__create_session(env: { "DEBUG": "1" }, cwd: "/project")
← { session_id: "sess_jkl012", pid: 12348, program: "/bin/zsh", ... }
```

---

#### terminal__destroy_session

Terminate a session and clean up resources.

```yaml
Input:
  session_id: string            # Session to destroy
  force: bool = false           # true = SIGKILL, false = SIGTERM then wait

Output:
  destroyed: bool               # Whether session was successfully destroyed
  exit_code: int | null         # Exit code if process terminated gracefully
```

---

#### terminal__list_sessions

List all active sessions.

```yaml
Input: (none)

Output:
  sessions: [
    {
      session_id: string
      program: string           # Running program
      args: [string]            # Program arguments
      pid: int
      created_at: string        # ISO 8601 timestamp
      dimensions: { rows, cols }
      exited: bool              # Whether process has exited
      exit_code: int | null     # Exit code if exited
      healthy: bool             # No errors, not exited
    }
  ]
  count: int                    # Number of active sessions
```

---

### Session Interaction Tools

All require `session_id` to identify which session to interact with.

#### terminal__send

Send input to a session, optionally reading output after.

```yaml
Input:
  session_id: string            # Required - target session

  # Input (one of text or key required)
  text: string                  # Raw text to send
  key: enum                     # Special key (see below)

  # Modifiers (for key)
  ctrl: bool = false
  alt: bool = false
  shift: bool = false

  # Paste mode
  bracketed_paste: bool | "auto" = "auto"
    # auto = enabled for multi-line text
    # Wraps text in \x1b[200~ ... \x1b[201~

  # Optional: read output after sending (reduces round trips)
  read: object                  # terminal__read parameters (minus session_id)
    view: ...
    format: ...
    timeout_ms: ...
    wait_idle_ms: ...
    wait_for_prompt: ...

Output:
  sent: bool
  read_result: object | null    # Present if read was provided

Keys:
  Navigation: up, down, left, right, home, end, pageup, pagedown
  Editing: backspace, delete, insert, tab
  Control: enter, escape
  Function: f1, f2, ... f12
  # Ctrl+C, Ctrl+D, Ctrl+Z via ctrl: true + key: "c"/"d"/"z"
```

---

#### terminal__read

Read output from a session.

```yaml
Input:
  session_id: string            # Required - target session

  view: enum
    "screen"      # Current visible buffer (rows x cols) - for TUI apps
    "new"         # All output since last read - for command output
    "scrollback"  # Historical output with pagination - for review

  format: enum
    "plain"       # ANSI codes stripped
    "raw"         # Preserve ANSI codes

  # Wait conditions (useful for knowing "when done")
  timeout_ms: int = 0           # Max wait time (0 = immediate)
  wait_idle_ms: int = 0         # Wait until no output for N ms
  wait_for_prompt: bool = false # Wait for shell prompt (uses config pattern)

  # Pagination (scrollback view only)
  offset: int = 0               # Lines from end (0 = most recent)
  limit: int = 1000             # Max lines to return

Output:
  content: string               # Terminal content
  lines: int                    # Number of lines in content
  cursor: { row, col } | null   # Cursor position (screen view only)
  dimensions: { rows, cols }    # Terminal size

  has_new_content: bool         # New content since last read
  prompt_detected: bool         # Shell prompt was detected
  idle: bool                    # No output for wait_idle_ms

  exited: bool                  # Process has exited
  exit_code: int | null         # Exit code if exited
```

---

#### terminal__get_info

Get session state without reading content.

```yaml
Input:
  session_id: string            # Required - target session

Output:
  session_id: string
  program: string
  args: [string]
  pid: int
  created_at: string
  cursor: { row, col }
  dimensions: { rows, cols }
  exited: bool
  exit_code: int | null
  healthy: bool                 # No errors, not exited
  cwd: string | null            # Current working directory (if detectable)
```

---

## Component Specifications

### 1. SessionManager (`session/manager.rs`)

Manages multiple terminal sessions with per-session locking.

```rust
pub struct SessionManager {
    sessions: RwLock<HashMap<String, Arc<Mutex<TerminalSession>>>>,
    config: GlobalConfig,
}

impl SessionManager {
    pub fn new(config: GlobalConfig) -> Self;

    /// Create a new session, returns session ID
    pub async fn create_session(&self, opts: CreateSessionOptions) -> Result<String>;

    /// Destroy a session by ID
    pub async fn destroy_session(&self, id: &str, force: bool) -> Result<DestroyResult>;

    /// Get session for independent locking
    pub fn get(&self, id: &str) -> Result<Arc<Mutex<TerminalSession>>>;

    /// List all sessions
    pub fn list(&self) -> Vec<SessionInfo>;

    /// Count active sessions
    pub fn count(&self) -> usize;

    /// Shutdown all sessions (for graceful exit)
    pub async fn shutdown(&self);
}

impl Drop for SessionManager {
    fn drop(&mut self) {
        // Terminate all sessions on drop
    }
}
```

### 2. TerminalSession (`session/session.rs`)

Wrapper around TerminalState with session metadata.

```rust
pub struct TerminalSession {
    pub id: String,
    pub program: String,
    pub args: Vec<String>,
    pub created_at: Instant,
    pub state: TerminalState,
    pub reader: SessionReader,
    pub error: Option<String>,  // Set if fatal error occurred
}

pub struct CreateSessionOptions {
    pub program: String,
    pub args: Vec<String>,
    pub rows: u16,
    pub cols: u16,
    pub env: HashMap<String, String>,
    pub cwd: Option<PathBuf>,
    pub wait_ready: Option<bool>,
    pub ready_timeout_ms: u64,
}

impl TerminalSession {
    pub fn new(id: String, opts: CreateSessionOptions, config: &GlobalConfig) -> Result<Self>;

    pub fn info(&self) -> SessionInfo;

    pub fn is_healthy(&self) -> bool {
        self.error.is_none() && !self.state.exited()
    }

    pub fn terminate(&mut self, force: bool) -> Result<Option<i32>>;
}
```

### 3. SessionReader (`session/reader.rs`)

Background thread that continuously reads PTY output.

```rust
pub struct SessionReader {
    handle: Option<JoinHandle<()>>,
    rx: mpsc::Receiver<ReaderMessage>,
    shutdown: Arc<AtomicBool>,
}

pub enum ReaderMessage {
    Data(Vec<u8>),
    Exited(Option<i32>),
    Error(String),
}

impl SessionReader {
    /// Spawn reader thread for the given PTY reader
    pub fn spawn(pty_reader: Box<dyn Read + Send>, child: Box<dyn Child>) -> Self;

    /// Drain all available messages, process through state
    pub async fn drain(&mut self, state: &mut TerminalState) -> Result<()>;

    /// Check if there are pending messages without blocking
    pub fn has_pending(&self) -> bool;

    /// Signal shutdown
    pub fn shutdown(&self);
}

impl Drop for SessionReader {
    fn drop(&mut self) {
        self.shutdown();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
```

### 4. Session ID Generation (`session/id.rs`)

```rust
pub fn generate_session_id() -> String {
    // Format: "sess_" + 8 random alphanumeric chars
    // e.g., "sess_a1b2c3d4"
    let suffix: String = uuid::Uuid::new_v4()
        .to_string()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .take(8)
        .collect();
    format!("sess_{}", suffix)
}
```

### 5. PtySession (`pty/session.rs`)

Manages the PTY master/slave pair and process lifecycle.

```rust
pub struct PtySession {
    master: Box<dyn MasterPty>,
    child: Box<dyn Child>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    size: TerminalSize,
}

impl PtySession {
    /// Spawn program with given options
    pub fn new(opts: &PtyOptions) -> Result<(Self, Box<dyn Read + Send>)>;

    /// Write bytes to PTY (send input) - async via spawn_blocking
    pub async fn write_async(&self, data: &[u8]) -> Result<()>;

    /// Check if child process is still running
    pub fn is_alive(&self) -> bool;

    /// Get exit code if terminated
    pub fn exit_code(&self) -> Option<i32>;

    /// Get child PID
    pub fn pid(&self) -> u32;

    /// Send signal to child
    pub fn signal(&mut self, sig: Signal) -> Result<()>;

    /// Terminate child (SIGTERM, then SIGKILL after timeout)
    pub fn terminate(&mut self, force: bool) -> Result<Option<i32>>;

    /// Get dimensions
    pub fn size(&self) -> TerminalSize;
}

pub struct PtyOptions {
    pub program: String,
    pub args: Vec<String>,
    pub rows: u16,
    pub cols: u16,
    pub env: HashMap<String, String>,
    pub cwd: Option<PathBuf>,
    pub term: String,
}
```

### 6. Environment Filtering (`pty/env.rs`)

```rust
/// Build environment for spawned process, filtering sensitive variables
pub fn build_environment(
    extra: &HashMap<String, String>,
    term: &str,
) -> HashMap<String, String> {
    let mut env: HashMap<String, String> = std::env::vars()
        .filter(|(k, _)| !is_sensitive_var(k))
        .collect();

    // Set TERM
    env.insert("TERM".to_string(), term.to_string());

    // Add user-provided vars (can override)
    env.extend(extra.clone());

    env
}

fn is_sensitive_var(name: &str) -> bool {
    matches!(name,
        "SSH_AUTH_SOCK" |
        "SSH_AGENT_PID" |
        "GPG_AGENT_INFO" |
        "AWS_SECRET_ACCESS_KEY" |
        "AWS_SESSION_TOKEN" |
        "GITHUB_TOKEN" |
        "ANTHROPIC_API_KEY" |
        "OPENAI_API_KEY"
    ) || name.contains("SECRET")
      || name.contains("PASSWORD")
      || name.contains("CREDENTIAL")
}
```

### 7. TerminalState (`terminal/state.rs`)

Per-session state that coordinates terminal emulation components.

```rust
pub struct TerminalState {
    pty: PtySession,
    screen: ScreenBuffer,
    scrollback: ScrollbackBuffer,
    output_tracker: OutputTracker,
    prompt_detector: PromptDetector,
    vt_parser: vte::Parser,
    rows: u16,
    cols: u16,
    exited: bool,
    exit_code: Option<i32>,
}

impl TerminalState {
    pub fn new(pty: PtySession, config: &SessionConfig) -> Result<Self>;

    /// Process raw PTY output through VT parser
    pub fn process_output(&mut self, data: &[u8]);

    /// Mark as exited
    pub fn set_exited(&mut self, code: Option<i32>);

    /// Get screen content
    pub fn screen(&self) -> &ScreenBuffer;

    /// Get mutable PTY access
    pub fn pty(&self) -> &PtySession;

    /// Get output tracker
    pub fn tracker_mut(&mut self) -> &mut OutputTracker;

    pub fn exited(&self) -> bool;
    pub fn exit_code(&self) -> Option<i32>;
    pub fn dimensions(&self) -> Dimensions;
}
```

### 8. ScreenBuffer (`terminal/screen.rs`)

Represents the visible terminal screen with cursor tracking and wide character support.

```rust
pub struct ScreenBuffer {
    cells: Vec<Vec<Cell>>,       // rows x cols grid
    cursor: CursorState,
    rows: u16,
    cols: u16,
    current_attrs: CellAttributes,
    scrolled_lines: Vec<ScrollbackLine>,  // Lines to push to scrollback
    alternate_active: bool,      // Alternate screen buffer mode
    title: Option<String>,       // Window title from OSC
}

pub struct Cell {
    character: char,
    width: u8,                   // 0 = continuation, 1 = normal, 2 = wide
    attrs: CellAttributes,
}

pub struct CursorState {
    row: u16,
    col: u16,
    visible: bool,
}

impl ScreenBuffer {
    pub fn new(rows: u16, cols: u16) -> Self;

    /// Put character at cursor, handling wide characters
    pub fn put_char(&mut self, c: char);

    /// Get screen content as string (with or without ANSI)
    pub fn render(&self, format: OutputFormat) -> String;

    /// Get cursor position
    pub fn cursor(&self) -> CursorPosition;

    /// Take lines that scrolled off (for scrollback buffer)
    pub fn take_scrolled_lines(&mut self) -> Vec<ScrollbackLine>;

    /// Set/get title
    pub fn set_title(&mut self, title: String);
    pub fn title(&self) -> Option<&str>;
}
```

**Note**: Resize is not supported mid-session. Dimensions are fixed at creation.

### 9. VT Emulator (`terminal/emulator.rs`)

Implements `vte::Perform` trait with phased feature support.

```rust
pub struct ScreenPerformer<'a> {
    screen: &'a mut ScreenBuffer,
    scrollback: &'a mut ScrollbackBuffer,
    tracker: &'a mut OutputTracker,
}

impl vte::Perform for ScreenPerformer<'_> {
    // Phase 1: Core functionality
    fn print(&mut self, c: char);           // Character output
    fn execute(&mut self, byte: u8);        // Control chars (BS, HT, LF, CR)

    // Phase 2: CSI sequences
    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], ignore: bool, action: char);
        // Cursor: A, B, C, D, H, f, G, d
        // Erase: J, K
        // Scroll: S, T
        // SGR: m

    // Phase 3: Mode handling
    fn esc_dispatch(&mut self, intermediates: &[u8], ignore: bool, byte: u8);
        // Save/restore cursor: 7, 8

    // Phase 4: Private modes (DECSET/DECRST)
    // Alternate screen: 1049, 47, 1047

    // OSC sequences - capture title, ignore others
    fn osc_dispatch(&mut self, params: &[&[u8]], bell_terminated: bool);

    // Ignore unknown sequences gracefully
    fn hook(&mut self, ...) {}
    fn unhook(&mut self) {}
    fn put(&mut self, _byte: u8) {}
}
```

### 10. ScrollbackBuffer (`terminal/scrollback.rs`)

Ring buffer for lines that scroll off the top of the screen.

```rust
pub struct ScrollbackBuffer {
    lines: VecDeque<ScrollbackLine>,
    max_lines: usize,
}

pub struct ScrollbackLine {
    plain: String,
    raw: String,                 // With ANSI codes
}

impl ScrollbackBuffer {
    pub fn new(max_lines: usize) -> Self;

    /// Push lines that scrolled off screen
    pub fn push(&mut self, lines: Vec<ScrollbackLine>);

    /// Get lines with pagination (0 = most recent)
    pub fn get(&self, offset: usize, limit: usize, format: OutputFormat) -> String;

    /// Total lines stored
    pub fn len(&self) -> usize;

    /// Clear buffer
    pub fn clear(&mut self);
}
```

### 11. OutputTracker (`terminal/tracker.rs`)

Tracks output for the "new" view mode - everything since last `terminal__read`.

```rust
pub struct OutputTracker {
    buffer: Vec<u8>,             // Raw bytes since last read
}

impl OutputTracker {
    pub fn new() -> Self;

    /// Append new PTY output
    pub fn append(&mut self, data: &[u8]);

    /// Get and clear tracked output
    pub fn take(&mut self, format: OutputFormat) -> String;

    /// Peek without clearing
    pub fn peek(&self, format: OutputFormat) -> String;

    /// Check if there's new content
    pub fn has_content(&self) -> bool;

    /// Clear without returning
    pub fn clear(&mut self);
}
```

### 12. PromptDetector (`terminal/prompt.rs`)

Detects shell prompt using configurable patterns.

```rust
pub struct PromptDetector {
    pattern: Regex,
}

impl PromptDetector {
    pub fn new(pattern: &str) -> Result<Self>;

    /// Check if content ends with shell prompt
    pub fn detect(&self, content: &str) -> bool;
}
```

Default pattern: `\$\s*$|#\s*$|>\s*$` (matches `$ `, `# `, `> ` at end)

Note: `wait_for_prompt` is most useful when running a shell. For non-shell programs, it simply won't match.

### 13. Input Encoding (`input/keys.rs`)

Encode special keys and modifiers to escape sequences.

```rust
pub enum SpecialKey {
    Up, Down, Left, Right,
    Home, End, PageUp, PageDown,
    Insert, Delete, Backspace, Tab,
    Enter, Escape,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
}

pub struct KeyInput {
    pub key: Option<SpecialKey>,
    pub text: Option<String>,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl KeyInput {
    /// Encode to bytes to send to PTY
    pub fn encode(&self) -> Result<Vec<u8>>;
}
```

**Encoding Table** (xterm-style):
| Key | Sequence |
|-----|----------|
| Up | `\x1b[A` |
| Down | `\x1b[B` |
| Right | `\x1b[C` |
| Left | `\x1b[D` |
| Home | `\x1b[H` |
| End | `\x1b[F` |
| PageUp | `\x1b[5~` |
| PageDown | `\x1b[6~` |
| Insert | `\x1b[2~` |
| Delete | `\x1b[3~` |
| F1-F4 | `\x1bOP` - `\x1bOS` |
| F5-F12 | `\x1b[15~` - `\x1b[24~` |
| Ctrl+C | `\x03` |
| Ctrl+D | `\x04` |
| Ctrl+Z | `\x1a` |

**Modifiers**: CSI sequences with modifier codes:
- `\x1b[1;2A` = Shift+Up (modifier 2)
- `\x1b[1;5A` = Ctrl+Up (modifier 5)
- `\x1b[1;3A` = Alt+Up (modifier 3)

### 14. Bracketed Paste (`input/paste.rs`)

```rust
pub enum BracketedPasteMode {
    Always,
    Never,
    Auto,  // Enabled for multi-line text
}

pub fn wrap_bracketed_paste(text: &str) -> Vec<u8> {
    let mut result = Vec::new();
    result.extend_from_slice(b"\x1b[200~");  // Start
    result.extend_from_slice(text.as_bytes());
    result.extend_from_slice(b"\x1b[201~");  // End
    result
}

pub fn should_use_bracketed_paste(text: &str, mode: BracketedPasteMode) -> bool {
    match mode {
        BracketedPasteMode::Always => true,
        BracketedPasteMode::Never => false,
        BracketedPasteMode::Auto => text.contains('\n'),
    }
}
```

### 15. ANSI Stripping (`output/ansi.rs`)

Strip ANSI escape codes for plain text output.

```rust
pub fn strip_ansi(input: &[u8]) -> String {
    // Use vte parser to remove:
    // - CSI sequences: \x1b[...m, \x1b[...H, etc.
    // - OSC sequences: \x1b]....\x07
    // - Simple escapes: \x1b[?..., \x1b=, etc.
}
```

Consider using `strip-ansi-escapes` crate or implementing with `vte`.

---

## Configuration

### Global Config (from MCP user_config)

```rust
pub struct GlobalConfig {
    // Defaults for new sessions
    pub default_rows: u16,
    pub default_cols: u16,
    pub default_shell: String,
    pub term: String,
    pub scrollback_limit: usize,
    pub prompt_pattern: String,

    // Session management
    pub max_sessions: usize,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            default_rows: 24,
            default_cols: 80,
            default_shell: std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into()),
            term: "xterm-256color".into(),
            scrollback_limit: 10000,
            prompt_pattern: r"\$\s*$|#\s*$|>\s*$".into(),
            max_sessions: 10,
        }
    }
}
```

### manifest.json user_config

```json
{
  "user_config": {
    "default_rows": {
      "type": "integer",
      "title": "Default Terminal Rows",
      "description": "Default number of rows for new sessions (default: 24)",
      "default": 24
    },
    "default_cols": {
      "type": "integer",
      "title": "Default Terminal Columns",
      "description": "Default number of columns for new sessions (default: 80)",
      "default": 80
    },
    "default_shell": {
      "type": "string",
      "title": "Default Shell",
      "description": "Default shell for new sessions (default: $SHELL or /bin/bash)"
    },
    "term": {
      "type": "string",
      "title": "TERM Variable",
      "description": "Terminal type for TERM env var (default: xterm-256color)",
      "default": "xterm-256color"
    },
    "scrollback_limit": {
      "type": "integer",
      "title": "Scrollback Limit",
      "description": "Maximum lines to keep in scrollback per session (default: 10000)",
      "default": 10000
    },
    "prompt_pattern": {
      "type": "string",
      "title": "Prompt Pattern",
      "description": "Regex pattern to detect shell prompt (for wait_for_prompt)",
      "default": "\\$\\s*$|#\\s*$|>\\s*$"
    },
    "max_sessions": {
      "type": "integer",
      "title": "Maximum Sessions",
      "description": "Maximum number of concurrent sessions (default: 10)",
      "default": 10
    }
  }
}
```

---

## MCP Server Implementation

```rust
// lib.rs
#![cfg(unix)]

// bin/main.rs
use std::sync::Arc;
use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};
use terminal::Server;
use tracing_subscriber::{self, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    // Logging to stderr only (stdout is for MCP)
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::DEBUG.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    // Graceful shutdown handler
    let shutdown = Arc::new(tokio::sync::Notify::new());
    let shutdown_clone = shutdown.clone();

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        tracing::info!("Received shutdown signal");
        shutdown_clone.notify_one();
    });

    // Run server
    let server = Server::new()?;
    let service = server.serve(stdio()).await?;

    tokio::select! {
        result = service.waiting() => result?,
        _ = shutdown.notified() => {
            tracing::info!("Shutting down, cleaning up sessions");
            server.shutdown().await;
        }
    }

    Ok(())
}
```

---

## Wait Condition Implementation

```rust
pub struct WaitCondition {
    pub timeout_ms: u64,
    pub wait_idle_ms: u64,
    pub wait_for_prompt: bool,
}

pub async fn wait_for_condition(
    session: &mut TerminalSession,
    condition: WaitCondition,
) -> Result<()> {
    let deadline = Instant::now() + Duration::from_millis(condition.timeout_ms.max(1));
    let mut last_output = Instant::now();

    loop {
        // Drain available output from reader
        let had_data = session.reader.drain(&mut session.state).await?;
        if had_data {
            last_output = Instant::now();
        }

        // Check exit
        if session.state.exited() {
            break;
        }

        // Check prompt
        if condition.wait_for_prompt {
            let content = session.state.tracker().peek(OutputFormat::Plain);
            if session.state.prompt_detector().detect(&content) {
                break;
            }
        }

        // Check idle
        if condition.wait_idle_ms > 0
            && last_output.elapsed() >= Duration::from_millis(condition.wait_idle_ms)
        {
            break;
        }

        // Check timeout
        if Instant::now() >= deadline {
            break;
        }

        // Small sleep to avoid busy loop
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    Ok(())
}
```

---

## Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum TerminalError {
    #[error("PTY error: {0}")]
    Pty(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Maximum sessions reached ({0})")]
    MaxSessionsReached(usize),

    #[error("Session already destroyed: {0}")]
    SessionDestroyed(String),

    #[error("Session has error: {0}")]
    SessionError(String),

    #[error("No input provided (need text or key)")]
    NoInput,

    #[error("Invalid key: {0}")]
    InvalidKey(String),

    #[error("Invalid prompt pattern: {0}")]
    InvalidPattern(#[from] regex::Error),

    #[error("Process has exited with code {0:?}")]
    ProcessExited(Option<i32>),

    #[error("Program not found: {0}")]
    ProgramNotFound(String),

    #[error("Wait timeout after {0}ms")]
    WaitTimeout(u64),
}

impl TerminalError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Pty(_) => "PTY_ERROR",
            Self::Io(_) => "IO_ERROR",
            Self::SessionNotFound(_) => "SESSION_NOT_FOUND",
            Self::MaxSessionsReached(_) => "MAX_SESSIONS",
            Self::SessionDestroyed(_) => "SESSION_DESTROYED",
            Self::SessionError(_) => "SESSION_ERROR",
            Self::NoInput => "NO_INPUT",
            Self::InvalidKey(_) => "INVALID_KEY",
            Self::InvalidPattern(_) => "INVALID_PATTERN",
            Self::ProcessExited(_) => "PROCESS_EXITED",
            Self::ProgramNotFound(_) => "PROGRAM_NOT_FOUND",
            Self::WaitTimeout(_) => "WAIT_TIMEOUT",
        }
    }
}
```

---

## Example Flows

### Create shell, run commands

```yaml
# Create default shell session
→ terminal__create_session()
← { session_id: "sess_abc123", pid: 12345, program: "/bin/bash" }

# Run command with output
→ terminal__send(
    session_id: "sess_abc123",
    text: "ls -la\n",
    read: { view: "new", wait_for_prompt: true, timeout_ms: 5000 }
  )
← { sent: true, read_result: { content: "total 48\n...\n$ ", prompt_detected: true } }

# Another command
→ terminal__send(
    session_id: "sess_abc123",
    text: "pwd\n",
    read: { view: "new", wait_for_prompt: true }
  )
← { sent: true, read_result: { content: "/home/user\n$ ", prompt_detected: true } }
```

### Interactive TUI app

```yaml
# Create vim session directly
→ terminal__create_session(program: "vim", args: ["file.txt"])
← { session_id: "sess_vim001", pid: 12346, program: "/usr/bin/vim" }

# Read initial screen
→ terminal__read(session_id: "sess_vim001", view: "screen", wait_idle_ms: 200)
← { content: "<vim screen>", cursor: { row: 1, col: 1 } }

# Navigate down
→ terminal__send(session_id: "sess_vim001", key: "down", read: { view: "screen", wait_idle_ms: 50 })
← { sent: true, read_result: { content: "...", cursor: { row: 2, col: 1 } } }

# Enter insert mode, type text
→ terminal__send(session_id: "sess_vim001", text: "ihello world")
→ terminal__send(session_id: "sess_vim001", key: "escape")

# Save and quit
→ terminal__send(session_id: "sess_vim001", text: ":wq\n", read: { view: "new", timeout_ms: 1000 })
← { sent: true, read_result: { exited: true, exit_code: 0 } }

# Clean up (optional - session auto-removed when process exits)
→ terminal__destroy_session(session_id: "sess_vim001")
```

### Multiple concurrent sessions

```yaml
# Create two shells
→ terminal__create_session(cwd: "/project-a")
← { session_id: "sess_projA", ... }

→ terminal__create_session(cwd: "/project-b")
← { session_id: "sess_projB", ... }

# Work in both
→ terminal__send(session_id: "sess_projA", text: "npm run build\n")
→ terminal__send(session_id: "sess_projB", text: "cargo build\n")

# Check status
→ terminal__list_sessions()
← { sessions: [{ session_id: "sess_projA", ... }, { session_id: "sess_projB", ... }], count: 2 }

# Read outputs
→ terminal__read(session_id: "sess_projA", view: "new", wait_idle_ms: 1000)
→ terminal__read(session_id: "sess_projB", view: "new", wait_idle_ms: 1000)
```

### Interrupt and control

```yaml
# Start long-running command
→ terminal__send(session_id: "sess_abc123", text: "sleep 3600\n")

# Interrupt it
→ terminal__send(
    session_id: "sess_abc123",
    key: "c",
    ctrl: true,
    read: { view: "new", wait_for_prompt: true }
  )
← { sent: true, read_result: { content: "^C\n$ ", prompt_detected: true } }
```

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    // Session ID generation
    #[test]
    fn test_session_id_format();
    #[test]
    fn test_session_id_uniqueness();

    // Screen buffer tests
    #[test]
    fn test_screen_cursor_movement();
    #[test]
    fn test_screen_scroll();
    #[test]
    fn test_screen_erase();
    #[test]
    fn test_alternate_screen_buffer();
    #[test]
    fn test_wide_character_handling();

    // Input encoding tests
    #[test]
    fn test_special_key_encoding();
    #[test]
    fn test_ctrl_modifier();
    #[test]
    fn test_bracketed_paste();

    // ANSI stripping tests
    #[test]
    fn test_strip_colors();
    #[test]
    fn test_strip_cursor_movement();

    // Prompt detection tests
    #[test]
    fn test_bash_prompt();
    #[test]
    fn test_zsh_prompt();
    #[test]
    fn test_custom_prompt_pattern();

    // Environment filtering
    #[test]
    fn test_sensitive_vars_filtered();
    #[test]
    fn test_term_set();
}
```

### Integration Tests

```rust
// tests/integration.rs

#[tokio::test]
async fn test_create_destroy_session() {
    let mgr = SessionManager::new(GlobalConfig::default());

    let id = mgr.create_session(CreateSessionOptions::default()).await.unwrap();
    assert!(mgr.get(&id).is_ok());

    mgr.destroy_session(&id, false).await.unwrap();
    assert!(mgr.get(&id).is_err());
}

#[tokio::test]
async fn test_max_sessions_limit() {
    let mut config = GlobalConfig::default();
    config.max_sessions = 2;
    let mgr = SessionManager::new(config);

    mgr.create_session(CreateSessionOptions::default()).await.unwrap();
    mgr.create_session(CreateSessionOptions::default()).await.unwrap();

    let result = mgr.create_session(CreateSessionOptions::default()).await;
    assert!(matches!(result, Err(TerminalError::MaxSessionsReached(2))));
}

#[tokio::test]
async fn test_simple_command() {
    let mgr = SessionManager::new(GlobalConfig::default());
    let id = mgr.create_session(CreateSessionOptions::default()).await.unwrap();

    let session = mgr.get(&id).unwrap();
    let mut session = session.lock().await;

    // Send echo command
    session.state.pty().write_async(b"echo hello\n").await.unwrap();

    // Wait and read
    tokio::time::sleep(Duration::from_millis(100)).await;
    session.reader.drain(&mut session.state).await.unwrap();
    // ... verify output contains "hello"
}

#[tokio::test]
async fn test_non_shell_program() {
    let mgr = SessionManager::new(GlobalConfig::default());
    let id = mgr.create_session(CreateSessionOptions {
        program: "cat".into(),
        args: vec![],
        wait_ready: Some(false),
        ..Default::default()
    }).await.unwrap();

    let session = mgr.get(&id).unwrap();
    let mut session = session.lock().await;
    session.state.pty().write_async(b"test input\n").await.unwrap();

    // cat should echo it back
    // ... verify output
}

#[tokio::test]
async fn test_ctrl_c_interrupt() {
    // Start sleep, send Ctrl+C, verify exit
}

#[tokio::test]
async fn test_graceful_shutdown() {
    let mgr = SessionManager::new(GlobalConfig::default());
    mgr.create_session(CreateSessionOptions::default()).await.unwrap();
    mgr.create_session(CreateSessionOptions::default()).await.unwrap();

    assert_eq!(mgr.count(), 2);

    mgr.shutdown().await;

    assert_eq!(mgr.count(), 0);
}
```

---

## Implementation Order

### Phase 1: Core Infrastructure
1. `types.rs` - Shared types, errors
2. `config.rs` - GlobalConfig
3. `session/id.rs` - Session ID generation
4. `pty/env.rs` - Environment filtering
5. `pty/session.rs` - Basic PTY creation, read/write
6. `session/reader.rs` - Background reader thread
7. `terminal/state.rs` - Basic TerminalState
8. `session/session.rs` - TerminalSession wrapper
9. `session/manager.rs` - SessionManager with per-session locking

### Phase 2: Basic Tools
1. `tools/create_session.rs` - Create session with wait_ready
2. `tools/destroy_session.rs` - Destroy session
3. `tools/list_sessions.rs` - List sessions
4. `tools/send.rs` - Basic send (text only)
5. `tools/read.rs` - Basic read (raw output, "new" view only)
6. `bin/main.rs` - MCP server wiring with graceful shutdown

### Phase 3: Terminal Emulation
1. `terminal/screen.rs` - Screen buffer with wide char support
2. `terminal/cursor.rs` - Cursor state
3. `terminal/emulator.rs` - VT parser performer (Phase 1: core)
4. `terminal/scrollback.rs` - Scrollback buffer
5. Screen view mode in `terminal__read`
6. Scrollback view mode

### Phase 4: Input Handling
1. `input/keys.rs` - Special key encoding
2. `input/modifiers.rs` - Ctrl/Alt/Shift
3. `input/paste.rs` - Bracketed paste
4. Full `terminal__send` implementation

### Phase 5: Output Processing
1. `output/ansi.rs` - ANSI stripping
2. `terminal/tracker.rs` - Output tracking for "new" view
3. Plain vs raw format support

### Phase 6: Wait Conditions
1. `terminal/prompt.rs` - Prompt detection
2. Wait condition loop implementation
3. `wait_for_prompt` support
4. `timeout_ms` and `wait_idle_ms`

### Phase 7: VT Emulator Completion
1. SGR attributes (colors, bold, etc.)
2. Scroll regions
3. Alternate screen buffer (DECSET 1049)
4. OSC title capture

### Phase 8: Polish
1. `tools/info.rs` - Get session info
2. CWD detection (platform-specific, best-effort)
3. Comprehensive error handling
4. Logging and debugging support
5. manifest.json finalization
6. Integration tests
7. Documentation

---

## Limitations

Documented limitations for v1:

1. **No resize mid-session** - Terminal dimensions are fixed at creation. Create a new session if different dimensions are needed.

2. **No content reflow** - Historical output maintains original line wrapping.

3. **Unix only** - macOS and Linux. Windows is not supported.

4. **No mouse input** - TUI apps requiring mouse interaction need keyboard navigation.

5. **Best-effort CWD detection** - Current working directory detection is platform-specific and may not work for all programs.

---

## Security Considerations

1. **Full PTY Access**: Each session gives full access to the spawned program
2. **Environment Filtering**: Sensitive variables (secrets, tokens) are filtered from inherited environment
3. **Resource Limits**:
   - `max_sessions` prevents session exhaustion
   - `scrollback_limit` prevents memory exhaustion per session
4. **Timeout Enforcement**: All waits have max timeout
5. **Process Isolation**: Each session is a separate process
6. **No File Access**: Tool only interacts via PTY I/O
7. **Stdout Isolation**: All logging goes to stderr; PTY output never touches stdout

---

## Performance Considerations

1. **Background Reader**: Dedicated thread per session avoids blocking async runtime
2. **Per-Session Locking**: Operations on different sessions don't block each other
3. **Output Processing**: VT parsing per-session; batch when possible
4. **Scrollback Memory**: Ring buffer with configurable limit per session
5. **Session Overhead**: Each session has its own PTY, buffers, parser, reader thread
6. **Async PTY Writes**: Using `spawn_blocking` to avoid blocking tokio
7. **Wait Loop**: 10ms sleep between checks to avoid busy waiting
