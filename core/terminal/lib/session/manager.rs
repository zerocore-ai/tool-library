//! Session manager for multiple terminal sessions.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};

use crate::config::GlobalConfig;
use crate::types::{Result, TerminalError};

use super::session::{CreateSessionOptions, SessionInfo, TerminalSession};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Result of destroying a session.
#[derive(Debug)]
pub struct DestroyResult {
    /// Whether the session was destroyed.
    pub destroyed: bool,

    /// Exit code if the process terminated gracefully.
    pub exit_code: Option<i32>,
}

/// Manages multiple terminal sessions.
pub struct SessionManager {
    /// Map of session ID to session.
    sessions: RwLock<HashMap<String, Arc<Mutex<TerminalSession>>>>,

    /// Global configuration.
    config: GlobalConfig,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl SessionManager {
    /// Create a new session manager.
    pub fn new(config: GlobalConfig) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            config,
        }
    }

    /// Create a new session.
    pub async fn create_session(&self, opts: CreateSessionOptions) -> Result<SessionInfo> {
        // Check session limit
        let count = self.sessions.read().await.len();
        if count >= self.config.max_sessions {
            return Err(TerminalError::MaxSessionsReached(self.config.max_sessions));
        }

        // Create the session
        let mut session = TerminalSession::new(opts, &self.config)?;

        // Start socket server for attachment support
        if let Err(e) = session.start_socket_server() {
            tracing::warn!("Failed to start socket server: {}", e);
            // Continue without socket support
        }

        let info = session.info();
        let id = session.id.clone();

        // Store it
        let session = Arc::new(Mutex::new(session));
        self.sessions.write().await.insert(id, session);

        Ok(info)
    }

    /// Get a session by ID.
    pub async fn get(&self, id: &str) -> Result<Arc<Mutex<TerminalSession>>> {
        self.sessions
            .read()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| TerminalError::SessionNotFound(id.to_string()))
    }

    /// Destroy a session.
    pub async fn destroy_session(&self, id: &str, force: bool) -> Result<DestroyResult> {
        // Get and remove the session
        let session = self
            .sessions
            .write()
            .await
            .remove(id)
            .ok_or_else(|| TerminalError::SessionNotFound(id.to_string()))?;

        // Terminate it
        let mut session = session.lock().await;
        let exit_code = session.terminate(force)?;

        Ok(DestroyResult {
            destroyed: true,
            exit_code,
        })
    }

    /// List all sessions.
    pub async fn list(&self) -> Vec<SessionInfo> {
        let sessions = self.sessions.read().await;
        let mut infos = Vec::with_capacity(sessions.len());

        for session in sessions.values() {
            let session = session.lock().await;
            infos.push(session.info());
        }

        infos
    }

    /// Count active sessions.
    pub async fn count(&self) -> usize {
        self.sessions.read().await.len()
    }

    /// Remove sessions whose processes have exited.
    pub async fn cleanup_exited(&self) -> Vec<String> {
        let mut to_remove = Vec::new();

        // Find exited sessions
        {
            let sessions = self.sessions.read().await;
            for (id, session) in sessions.iter() {
                let session = session.lock().await;
                if session.state.exited() {
                    to_remove.push(id.clone());
                }
            }
        }

        // Remove them
        if !to_remove.is_empty() {
            let mut sessions = self.sessions.write().await;
            for id in &to_remove {
                sessions.remove(id);
            }
        }

        to_remove
    }

    /// Shutdown all sessions.
    pub async fn shutdown(&self) {
        let mut sessions = self.sessions.write().await;

        for (id, session) in sessions.drain() {
            tracing::info!(session_id = %id, "Terminating session on shutdown");
            let mut session = session.lock().await;
            let _ = session.terminate(false);
        }
    }

    /// Get the global configuration.
    pub fn config(&self) -> &GlobalConfig {
        &self.config
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl Drop for SessionManager {
    fn drop(&mut self) {
        // Note: We can't async terminate here, but the sessions will clean up
        // their PTY processes when dropped.
        tracing::debug!("SessionManager dropped");
    }
}

impl std::fmt::Debug for SessionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionManager")
            .field("max_sessions", &self.config.max_sessions)
            .finish()
    }
}
