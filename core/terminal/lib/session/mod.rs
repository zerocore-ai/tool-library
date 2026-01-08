//! Session management.

mod id;
mod manager;
mod reader;
mod session;

pub use id::generate_session_id;
pub use manager::{DestroyResult, SessionManager};
pub use reader::{ReaderMessage, SessionReader};
pub use session::{is_shell_program, CreateSessionOptions, SessionInfo, TerminalSession};
