//! PTY session management.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

use crate::types::{Dimensions, Result, TerminalError};

use super::env::build_environment;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Options for creating a PTY session.
#[derive(Debug, Clone)]
pub struct PtyOptions {
    /// Program to run.
    pub program: String,

    /// Program arguments.
    pub args: Vec<String>,

    /// Terminal rows.
    pub rows: u16,

    /// Terminal columns.
    pub cols: u16,

    /// Additional environment variables.
    pub env: HashMap<String, String>,

    /// Working directory.
    pub cwd: Option<PathBuf>,

    /// Terminal type (TERM variable).
    pub term: String,
}

impl Default for PtyOptions {
    fn default() -> Self {
        Self {
            program: std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into()),
            args: Vec::new(),
            rows: 24,
            cols: 80,
            env: HashMap::new(),
            cwd: None,
            term: "xterm-256color".into(),
        }
    }
}

/// PTY session that manages the master/slave pair and child process.
pub struct PtySession {
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    size: Dimensions,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl PtySession {
    /// Spawn a new PTY session with the given options.
    ///
    /// Returns the session and a reader for the PTY output.
    pub fn new(opts: &PtyOptions) -> Result<(Self, Box<dyn Read + Send>)> {
        let pty_system = native_pty_system();

        let pty_size = PtySize {
            rows: opts.rows,
            cols: opts.cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = pty_system
            .openpty(pty_size)
            .map_err(|e| TerminalError::Pty(e.to_string()))?;

        let mut cmd = CommandBuilder::new(&opts.program);
        cmd.args(&opts.args);

        // Set environment
        let env = build_environment(&opts.env, &opts.term);
        for (key, value) in env {
            cmd.env(key, value);
        }

        // Set working directory
        if let Some(ref cwd) = opts.cwd {
            cmd.cwd(cwd);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| TerminalError::Pty(e.to_string()))?;

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| TerminalError::Pty(e.to_string()))?;

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| TerminalError::Pty(e.to_string()))?;

        let session = Self {
            master: pair.master,
            child,
            writer: Arc::new(Mutex::new(writer)),
            size: Dimensions {
                rows: opts.rows,
                cols: opts.cols,
            },
        };

        Ok((session, reader))
    }

    /// Write bytes to PTY (send input) - synchronous version.
    pub fn write(&self, data: &[u8]) -> Result<()> {
        let mut writer = self.writer.lock().map_err(|_| {
            TerminalError::Pty("Failed to acquire writer lock".to_string())
        })?;
        writer.write_all(data)?;
        writer.flush()?;
        Ok(())
    }

    /// Get a clone of the writer handle for async operations.
    ///
    /// Use this to perform writes without holding a reference to PtySession
    /// across await points, which is necessary because MasterPty is not Sync.
    pub fn writer(&self) -> Arc<Mutex<Box<dyn Write + Send>>> {
        self.writer.clone()
    }

    /// Check if child process is still running.
    pub fn is_alive(&mut self) -> bool {
        self.child.try_wait().ok().flatten().is_none()
    }

    /// Get exit code if terminated.
    pub fn exit_code(&mut self) -> Option<i32> {
        self.child
            .try_wait()
            .ok()
            .flatten()
            .map(|status| status.exit_code() as i32)
    }

    /// Get child PID.
    pub fn pid(&self) -> Option<u32> {
        self.child.process_id()
    }

    /// Terminate child (SIGTERM, then SIGKILL after timeout if force).
    pub fn terminate(&mut self, force: bool) -> Result<Option<i32>> {
        if force {
            self.child
                .kill()
                .map_err(|e| TerminalError::Pty(e.to_string()))?;
        } else {
            // Try graceful termination first
            #[cfg(unix)]
            {
                if let Some(pid) = self.child.process_id() {
                    unsafe {
                        libc::kill(pid as i32, libc::SIGTERM);
                    }
                }
            }

            #[cfg(not(unix))]
            {
                self.child
                    .kill()
                    .map_err(|e| TerminalError::Pty(e.to_string()))?;
            }
        }

        // Wait for exit
        let status = self
            .child
            .wait()
            .map_err(|e| TerminalError::Pty(e.to_string()))?;

        Ok(Some(status.exit_code() as i32))
    }

    /// Get current terminal dimensions.
    pub fn size(&self) -> Dimensions {
        self.size
    }

    /// Get a reference to the master PTY (for resize operations if needed).
    pub fn master(&self) -> &dyn MasterPty {
        &*self.master
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl std::fmt::Debug for PtySession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PtySession")
            .field("size", &self.size)
            .field("pid", &self.child.process_id())
            .finish()
    }
}
