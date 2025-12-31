//! Wire protocol for socket communication.
//!
//! Simple length-prefixed message format:
//! ```text
//! ┌──────────┬────────────┬─────────────────┐
//! │ Type (1) │ Length (4) │ Payload (N)     │
//! └──────────┴────────────┴─────────────────┘
//! ```

use serde::{Deserialize, Serialize};

use crate::types::Dimensions;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// Message type: PTY output (server -> client).
pub const MSG_OUTPUT: u8 = 0x01;

/// Message type: PTY input (client -> server).
pub const MSG_INPUT: u8 = 0x02;

/// Message type: Terminal resize (client -> server).
pub const MSG_RESIZE: u8 = 0x03;

/// Message type: Session info (server -> client on connect).
pub const MSG_INFO: u8 = 0x04;

/// Message type: Session closing (either direction).
pub const MSG_CLOSE: u8 = 0x05;

/// Header size: 1 byte type + 4 bytes length.
pub const HEADER_SIZE: usize = 5;

/// Maximum payload size (16 MB).
pub const MAX_PAYLOAD_SIZE: u32 = 16 * 1024 * 1024;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Messages sent over the socket.
#[derive(Debug, Clone)]
pub enum Message {
    /// PTY output data (server -> client).
    Output(Vec<u8>),

    /// PTY input data (client -> server).
    Input(Vec<u8>),

    /// Terminal resize request (client -> server).
    Resize { rows: u16, cols: u16 },

    /// Session information (server -> client on connect).
    Info(SessionInfoPayload),

    /// Session is closing.
    Close(Option<String>),
}

/// Session info payload sent on client connect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfoPayload {
    /// Session ID.
    pub session_id: String,

    /// Program running in the session.
    pub program: String,

    /// Program arguments.
    pub args: Vec<String>,

    /// Process ID.
    pub pid: Option<u32>,

    /// Terminal dimensions.
    pub dimensions: Dimensions,

    /// Current screen content.
    pub screen: String,
}

/// Protocol error types.
#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Unknown message type: {0}")]
    UnknownType(u8),

    #[error("Payload too large: {0} bytes (max {MAX_PAYLOAD_SIZE})")]
    PayloadTooLarge(u32),

    #[error("Invalid payload: {0}")]
    InvalidPayload(String),

    #[error("Connection closed")]
    ConnectionClosed,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl Message {
    /// Get the message type byte.
    pub fn msg_type(&self) -> u8 {
        match self {
            Message::Output(_) => MSG_OUTPUT,
            Message::Input(_) => MSG_INPUT,
            Message::Resize { .. } => MSG_RESIZE,
            Message::Info(_) => MSG_INFO,
            Message::Close(_) => MSG_CLOSE,
        }
    }

    /// Encode the message to bytes.
    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        let payload = match self {
            Message::Output(data) => data.clone(),
            Message::Input(data) => data.clone(),
            Message::Resize { rows, cols } => {
                let mut buf = Vec::with_capacity(4);
                buf.extend_from_slice(&rows.to_be_bytes());
                buf.extend_from_slice(&cols.to_be_bytes());
                buf
            }
            Message::Info(info) => {
                serde_json::to_vec(info).map_err(|e| ProtocolError::InvalidPayload(e.to_string()))?
            }
            Message::Close(reason) => {
                if let Some(r) = reason {
                    r.as_bytes().to_vec()
                } else {
                    Vec::new()
                }
            }
        };

        let len = payload.len() as u32;
        if len > MAX_PAYLOAD_SIZE {
            return Err(ProtocolError::PayloadTooLarge(len));
        }

        let mut buf = Vec::with_capacity(HEADER_SIZE + payload.len());
        buf.push(self.msg_type());
        buf.extend_from_slice(&len.to_be_bytes());
        buf.extend_from_slice(&payload);

        Ok(buf)
    }

    /// Decode a message from type and payload.
    pub fn decode(msg_type: u8, payload: Vec<u8>) -> Result<Self, ProtocolError> {
        match msg_type {
            MSG_OUTPUT => Ok(Message::Output(payload)),
            MSG_INPUT => Ok(Message::Input(payload)),
            MSG_RESIZE => {
                if payload.len() != 4 {
                    return Err(ProtocolError::InvalidPayload(
                        "Resize payload must be 4 bytes".into(),
                    ));
                }
                let rows = u16::from_be_bytes([payload[0], payload[1]]);
                let cols = u16::from_be_bytes([payload[2], payload[3]]);
                Ok(Message::Resize { rows, cols })
            }
            MSG_INFO => {
                let info: SessionInfoPayload = serde_json::from_slice(&payload)
                    .map_err(|e| ProtocolError::InvalidPayload(e.to_string()))?;
                Ok(Message::Info(info))
            }
            MSG_CLOSE => {
                let reason = if payload.is_empty() {
                    None
                } else {
                    Some(
                        String::from_utf8(payload)
                            .map_err(|e| ProtocolError::InvalidPayload(e.to_string()))?,
                    )
                };
                Ok(Message::Close(reason))
            }
            _ => Err(ProtocolError::UnknownType(msg_type)),
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Read a message from an async reader.
pub async fn read_message<R: tokio::io::AsyncReadExt + Unpin>(
    reader: &mut R,
) -> Result<Message, ProtocolError> {
    // Read header
    let mut header = [0u8; HEADER_SIZE];
    match reader.read_exact(&mut header).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
            return Err(ProtocolError::ConnectionClosed);
        }
        Err(e) => return Err(ProtocolError::Io(e)),
    }

    let msg_type = header[0];
    let len = u32::from_be_bytes([header[1], header[2], header[3], header[4]]);

    if len > MAX_PAYLOAD_SIZE {
        return Err(ProtocolError::PayloadTooLarge(len));
    }

    // Read payload
    let mut payload = vec![0u8; len as usize];
    if len > 0 {
        reader.read_exact(&mut payload).await?;
    }

    Message::decode(msg_type, payload)
}

/// Write a message to an async writer.
pub async fn write_message<W: tokio::io::AsyncWriteExt + Unpin>(
    writer: &mut W,
    msg: &Message,
) -> Result<(), ProtocolError> {
    let data = msg.encode()?;
    writer.write_all(&data).await?;
    writer.flush().await?;
    Ok(())
}
