//! terminal__send tool implementation.

use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ErrorData as McpError, Json};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::input::{encode_text, BracketedPasteMode, KeyInput, SpecialKey};
use crate::session::SessionManager;
use crate::types::TerminalError;

use super::read::{handle_read_internal, ReadInput, ReadOutput};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Input for send tool.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct SendInput {
    /// Session ID to send input to.
    pub session_id: String,

    /// Text to send.
    #[serde(default)]
    pub text: Option<String>,

    /// Special key to send.
    #[serde(default)]
    pub key: Option<SpecialKey>,

    /// Ctrl modifier.
    #[serde(default)]
    pub ctrl: bool,

    /// Alt modifier.
    #[serde(default)]
    pub alt: bool,

    /// Shift modifier.
    #[serde(default)]
    pub shift: bool,

    /// Bracketed paste mode (auto, always, never).
    #[serde(default)]
    pub bracketed_paste: BracketedPasteMode,

    /// Optional: read output after sending.
    #[serde(default)]
    pub read: Option<ReadOptions>,
}

/// Options for reading after send.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ReadOptions {
    /// View mode (screen, new, scrollback).
    #[serde(default)]
    pub view: Option<String>,

    /// Output format (plain, raw).
    #[serde(default)]
    pub format: Option<String>,

    /// Maximum wait time in milliseconds.
    #[serde(default)]
    pub timeout_ms: Option<u64>,

    /// Wait until no output for N milliseconds.
    #[serde(default)]
    pub wait_idle_ms: Option<u64>,

    /// Wait for shell prompt.
    #[serde(default)]
    pub wait_for_prompt: Option<bool>,

    /// Pagination offset (scrollback only).
    #[serde(default)]
    pub offset: Option<usize>,

    /// Pagination limit (scrollback only).
    #[serde(default)]
    pub limit: Option<usize>,
}

/// Output for send tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SendOutput {
    /// Whether the input was sent successfully.
    pub sent: bool,

    /// Read result if read options were provided.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_result: Option<ReadOutput>,
}

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Handle the send tool call.
pub async fn handle_send(
    manager: Arc<SessionManager>,
    params: Parameters<SendInput>,
) -> Result<Json<SendOutput>, McpError> {
    let input = params.0;

    // Get the session
    let session = manager
        .get(&input.session_id)
        .await
        .map_err(|e| e.to_mcp_error())?;

    // Build the input bytes
    let data = if let Some(key) = input.key {
        // Special key
        let key_input = KeyInput {
            key: Some(key),
            text: None,
            ctrl: input.ctrl,
            alt: input.alt,
            shift: input.shift,
        };
        key_input.encode().map_err(|e| e.to_mcp_error())?
    } else if let Some(ref text) = input.text {
        if input.ctrl || input.alt {
            // Text with modifiers
            let key_input = KeyInput {
                key: None,
                text: Some(text.clone()),
                ctrl: input.ctrl,
                alt: input.alt,
                shift: input.shift,
            };
            key_input.encode().map_err(|e| e.to_mcp_error())?
        } else {
            // Plain text, potentially with bracketed paste
            encode_text(text, input.bracketed_paste)
        }
    } else {
        return Err(TerminalError::NoInput.to_mcp_error());
    };

    // Send the input - get writer first, then drop lock before async operation
    let writer = {
        let session = session.lock().await;
        session.state.writer()
    };

    // Perform write in spawn_blocking since we can't hold &PtySession across await
    let data_owned = data;
    tokio::task::spawn_blocking(move || {
        use std::io::Write;
        let mut w = writer
            .lock()
            .map_err(|_| TerminalError::Pty("Failed to acquire writer lock".to_string()))?;
        w.write_all(&data_owned)?;
        w.flush()?;
        Ok::<_, TerminalError>(())
    })
    .await
    .map_err(|e| TerminalError::Pty(e.to_string()).to_mcp_error())?
    .map_err(|e| e.to_mcp_error())?;

    // Handle optional read
    let read_result = if let Some(read_opts) = input.read {
        let read_input = ReadInput {
            session_id: input.session_id.clone(),
            view: read_opts.view,
            format: read_opts.format,
            timeout_ms: read_opts.timeout_ms,
            wait_idle_ms: read_opts.wait_idle_ms,
            wait_for_prompt: read_opts.wait_for_prompt,
            offset: read_opts.offset,
            limit: read_opts.limit,
        };

        Some(handle_read_internal(manager, read_input).await?)
    } else {
        None
    };

    Ok(Json(SendOutput {
        sent: true,
        read_result,
    }))
}
