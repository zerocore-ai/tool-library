//! Unix socket support for session attachment.

pub mod protocol;
pub mod server;

pub use protocol::{read_message, write_message, Message, ProtocolError, SessionInfoPayload};
pub use server::{list_sockets, socket_path_for, SocketInput, SocketServer, SOCKET_DIR};
