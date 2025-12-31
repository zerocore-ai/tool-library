//! Terminal MCP Server
//!
//! An MCP server that provides AI agents with full pseudo-terminal (PTY) access,
//! enabling interaction with interactive terminal applications beyond simple command execution.
//!
//! # Features
//!
//! - **Multi-Session Support**: Create and manage multiple independent terminal sessions
//! - **PTY Access**: Full pseudo-terminal with proper terminal emulation
//! - **TUI Support**: Interact with TUI applications (vim, htop, etc.)
//! - **Shell Integration**: Run shells with job control and signal handling
//! - **Flexible Reading**: Screen view, new output, and scrollback history
//!
//! # Platform Support
//!
//! Unix only (macOS, Linux). Windows is not supported due to ConPTY differences.

#![cfg(unix)]

pub mod config;
pub mod input;
pub mod pty;
pub mod server;
pub mod session;
pub mod socket;
pub mod terminal;
pub mod tools;
pub mod types;

pub use config::GlobalConfig;
pub use server::Server;
pub use session::{SessionInfo, SessionManager};
pub use types::{CursorPosition, Dimensions, OutputFormat, Result, TerminalError, ViewMode};
