//! Terminal session wrapper.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::config::GlobalConfig;
use crate::pty::{PtyOptions, PtySession};
use crate::socket::{SocketInput, SocketServer};
use crate::terminal::TerminalState;
use crate::types::{CursorPosition, Dimensions, Result};

use super::id::generate_session_id;
use super::reader::{ReaderMessage, SessionReader};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Options for creating a new session.
#[derive(Debug, Clone, Default)]
pub struct CreateSessionOptions {
    /// Program to run (default: shell).
    pub program: Option<String>,

    /// Program arguments.
    pub args: Vec<String>,

    /// Terminal rows.
    pub rows: Option<u16>,

    /// Terminal columns.
    pub cols: Option<u16>,

    /// Additional environment variables.
    pub env: HashMap<String, String>,

    /// Working directory.
    pub cwd: Option<PathBuf>,

    /// Wait for shell to be ready before returning.
    pub wait_ready: Option<bool>,

    /// Timeout for wait_ready in milliseconds.
    pub ready_timeout_ms: Option<u64>,
}

/// Information about a session.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionInfo {
    /// Session identifier.
    pub session_id: String,

    /// Running program.
    pub program: String,

    /// Program arguments.
    pub args: Vec<String>,

    /// Process ID.
    pub pid: Option<u32>,

    /// Creation timestamp.
    pub created_at: DateTime<Utc>,

    /// Terminal dimensions.
    pub dimensions: Dimensions,

    /// Whether the process has exited.
    pub exited: bool,

    /// Exit code if exited.
    pub exit_code: Option<i32>,

    /// Whether the session is healthy (no errors, not exited).
    pub healthy: bool,

    /// Path to the Unix socket for attachment (if enabled).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub socket_path: Option<String>,

    /// Number of clients attached via socket.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attached_clients: Option<usize>,
}

/// A terminal session.
pub struct TerminalSession {
    /// Session identifier.
    pub id: String,

    /// Running program.
    pub program: String,

    /// Program arguments.
    pub args: Vec<String>,

    /// Creation time.
    pub created_at: Instant,

    /// Creation timestamp (for serialization).
    pub created_at_utc: DateTime<Utc>,

    /// Terminal state.
    pub state: TerminalState,

    /// Background reader.
    pub reader: SessionReader,

    /// Error message if a fatal error occurred.
    pub error: Option<String>,

    /// Socket server for external attachment.
    socket_server: Option<SocketServer>,

    /// Receiver for input from socket clients.
    socket_input_rx: Option<mpsc::Receiver<SocketInput>>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl TerminalSession {
    /// Create a new terminal session.
    pub fn new(opts: CreateSessionOptions, config: &GlobalConfig) -> Result<Self> {
        let id = generate_session_id();

        let program = opts
            .program
            .unwrap_or_else(|| config.default_shell.clone());
        let rows = opts.rows.unwrap_or(config.default_rows);
        let cols = opts.cols.unwrap_or(config.default_cols);

        let pty_opts = PtyOptions {
            program: program.clone(),
            args: opts.args.clone(),
            rows,
            cols,
            env: opts.env,
            cwd: opts.cwd,
            term: config.term.clone(),
        };

        let (pty, pty_reader) = PtySession::new(&pty_opts)?;
        let state = TerminalState::new(pty, config)?;
        let reader = SessionReader::spawn(pty_reader);

        Ok(Self {
            id,
            program,
            args: opts.args,
            created_at: Instant::now(),
            created_at_utc: Utc::now(),
            state,
            reader,
            error: None,
            socket_server: None,
            socket_input_rx: None,
        })
    }

    /// Start the socket server for this session, enabling external attachment.
    pub fn start_socket_server(&mut self) -> Result<()> {
        if self.socket_server.is_some() {
            return Ok(()); // Already started
        }

        let id = self.id.clone();
        let program = self.program.clone();
        let args = self.args.clone();
        let pid = self.state.pty().pid();
        let dimensions = self.state.dimensions();

        // We need a way to get the screen content. Since we can't clone TerminalState,
        // we'll just return an empty string and let clients get the initial screen
        // from the Info message.
        let screen_fn = move || String::new();

        let (server, input_rx) = SocketServer::start(
            id,
            program,
            args,
            pid,
            dimensions,
            screen_fn,
        )
        .map_err(|e| crate::types::TerminalError::Io(e))?;

        self.socket_server = Some(server);
        self.socket_input_rx = Some(input_rx);

        tracing::info!(session_id = %self.id, "Socket server started");
        Ok(())
    }

    /// Get the socket path if the socket server is running.
    pub fn socket_path(&self) -> Option<&Path> {
        self.socket_server.as_ref().map(|s| s.socket_path())
    }

    /// Get the number of attached clients.
    pub fn attached_client_count(&self) -> usize {
        self.socket_server
            .as_ref()
            .map(|s| s.client_count())
            .unwrap_or(0)
    }

    /// Broadcast output to attached clients.
    pub fn broadcast_output(&self, data: &[u8]) {
        if let Some(server) = &self.socket_server {
            server.broadcast_output(data);
        }
    }

    /// Drain input from socket clients and write to PTY.
    pub fn drain_socket_input(&mut self) -> Result<()> {
        if let Some(rx) = &mut self.socket_input_rx {
            while let Ok(input) = rx.try_recv() {
                match input {
                    SocketInput::Data(data) => {
                        self.state.pty().write(&data)?;
                    }
                    SocketInput::Resize { rows, cols } => {
                        // TODO: Implement resize if needed
                        tracing::debug!(rows, cols, "Resize request from socket client (not implemented)");
                    }
                }
            }
        }
        Ok(())
    }

    /// Get session information.
    pub fn info(&self) -> SessionInfo {
        SessionInfo {
            session_id: self.id.clone(),
            program: self.program.clone(),
            args: self.args.clone(),
            pid: self.state.pty().pid(),
            created_at: self.created_at_utc,
            dimensions: self.state.dimensions(),
            exited: self.state.exited(),
            exit_code: self.state.exit_code(),
            healthy: self.is_healthy(),
            socket_path: self.socket_path().map(|p| p.to_string_lossy().into_owned()),
            attached_clients: if self.socket_server.is_some() {
                Some(self.attached_client_count())
            } else {
                None
            },
        }
    }

    /// Check if the session is healthy.
    pub fn is_healthy(&self) -> bool {
        self.error.is_none() && !self.state.exited()
    }

    /// Get cursor position.
    pub fn cursor(&self) -> CursorPosition {
        self.state.cursor()
    }

    /// Terminate the session.
    pub fn terminate(&mut self, force: bool) -> Result<Option<i32>> {
        // Terminate PTY first - this causes the reader thread to get EOF
        let result = self.state.pty_mut().terminate(force);

        // Signal reader shutdown (it should already be exiting due to EOF)
        self.reader.shutdown();

        // Mark as exited
        if let Ok(code) = &result {
            self.state.set_exited(*code);
        }

        result
    }

    /// Process pending messages from the reader.
    pub fn drain_reader(&mut self) -> Result<bool> {
        // Also drain socket input
        let _ = self.drain_socket_input();

        let messages = self.reader.drain();
        let mut had_data = false;

        for msg in messages {
            match msg {
                ReaderMessage::Data(data) => {
                    // Broadcast to socket clients before processing
                    self.broadcast_output(&data);
                    self.state.process_output(&data);
                    had_data = true;
                }
                ReaderMessage::Exited(code) => {
                    self.state.set_exited(code);
                }
                ReaderMessage::Error(err) => {
                    self.error = Some(err);
                }
                ReaderMessage::Eof => {
                    // PTY closed, check if process exited
                    if let Some(code) = self.state.pty_mut().exit_code() {
                        self.state.set_exited(Some(code));
                    } else {
                        self.state.set_exited(None);
                    }
                }
            }
        }

        Ok(had_data)
    }

    /// Process pending messages with timeout.
    pub async fn drain_reader_async(&mut self, timeout_ms: u64) -> Result<bool> {
        use std::time::Duration;

        let mut had_data = false;
        let deadline = Instant::now() + Duration::from_millis(timeout_ms.max(1));

        loop {
            // Also drain socket input
            let _ = self.drain_socket_input();

            // First drain any immediately available messages
            if self.drain_reader()? {
                had_data = true;
            }

            // Check if we should stop
            if Instant::now() >= deadline {
                break;
            }

            // Wait for more data with timeout
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }

            let wait_time = remaining.min(Duration::from_millis(10));
            if let Some(msg) = self.reader.recv_timeout(wait_time).await {
                match msg {
                    ReaderMessage::Data(data) => {
                        // Broadcast to socket clients
                        self.broadcast_output(&data);
                        self.state.process_output(&data);
                        had_data = true;
                    }
                    ReaderMessage::Exited(code) => {
                        self.state.set_exited(code);
                        break;
                    }
                    ReaderMessage::Error(err) => {
                        self.error = Some(err);
                        break;
                    }
                    ReaderMessage::Eof => {
                        if let Some(code) = self.state.pty_mut().exit_code() {
                            self.state.set_exited(Some(code));
                        } else {
                            self.state.set_exited(None);
                        }
                        break;
                    }
                }
            }
        }

        Ok(had_data)
    }

    /// Shutdown the socket server if running.
    pub async fn shutdown_socket(&mut self) {
        if let Some(mut server) = self.socket_server.take() {
            server.shutdown().await;
        }
        self.socket_input_rx = None;
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl std::fmt::Debug for TerminalSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerminalSession")
            .field("id", &self.id)
            .field("program", &self.program)
            .field("args", &self.args)
            .field("healthy", &self.is_healthy())
            .field("error", &self.error)
            .finish()
    }
}

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Check if a program is a shell.
pub fn is_shell_program(program: &str) -> bool {
    let name = std::path::Path::new(program)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(program);

    matches!(
        name,
        "bash" | "zsh" | "sh" | "fish" | "dash" | "ksh" | "tcsh" | "csh" | "ash" | "pwsh"
    )
}
