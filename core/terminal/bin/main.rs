//! Terminal MCP Server and CLI.
//!
//! Usage:
//!   terminal                Run MCP server (default)
//!   terminal serve          Run MCP server (explicit)
//!   terminal list           List active sessions
//!   terminal attach <ID>    Attach to a session
//!   terminal info <ID>      Show session details

use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use rmcp::{transport::stdio, ServiceExt};
use tracing_subscriber::{self, EnvFilter};

use ::terminal::Server;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

const SOCKET_DIR: &str = "/tmp/terminal";
const HEADER_SIZE: usize = 5;

// Message types
const MSG_OUTPUT: u8 = 0x01;
const MSG_INPUT: u8 = 0x02;
const MSG_RESIZE: u8 = 0x03;
const MSG_INFO: u8 = 0x04;
const MSG_CLOSE: u8 = 0x05;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "terminal")]
#[command(about = "Terminal MCP server with session attachment support")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the MCP server (default if no command specified)
    Serve,

    /// List all active terminal sessions
    List,

    /// Attach to a terminal session
    Attach {
        /// Session ID (or prefix)
        session_id: String,
    },

    /// Show detailed information about a session
    Info {
        /// Session ID (or prefix)
        session_id: String,
    },
}

#[derive(Debug, serde::Deserialize)]
struct SessionInfoPayload {
    session_id: String,
    program: String,
    args: Vec<String>,
    pid: Option<u32>,
    dimensions: Dimensions,
    screen: String,
}

#[derive(Debug, serde::Deserialize)]
struct Dimensions {
    rows: u16,
    cols: u16,
}

//--------------------------------------------------------------------------------------------------
// Functions: Main
//--------------------------------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None | Some(Commands::Serve) => run_server().await,
        Some(Commands::List) => cmd_list().map_err(Into::into),
        Some(Commands::Attach { session_id }) => cmd_attach(&session_id).map_err(Into::into),
        Some(Commands::Info { session_id }) => cmd_info(&session_id).map_err(Into::into),
    }
}

//--------------------------------------------------------------------------------------------------
// Functions: MCP Server
//--------------------------------------------------------------------------------------------------

async fn run_server() -> Result<()> {
    // Logging to stderr only (stdout is reserved for MCP protocol)
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive(tracing::Level::DEBUG.into()),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("Starting Terminal MCP Server");

    // Create server
    let server = Server::new();
    let server_for_shutdown = server.clone();

    // Set up graceful shutdown
    let shutdown = Arc::new(tokio::sync::Notify::new());
    let shutdown_clone = shutdown.clone();

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        tracing::info!("Received shutdown signal");
        shutdown_clone.notify_one();
    });

    // Run the server
    let service = server.serve(stdio()).await?;

    tokio::select! {
        result = service.waiting() => {
            result?;
        }
        _ = shutdown.notified() => {
            tracing::info!("Shutting down, cleaning up sessions");
            server_for_shutdown.shutdown().await;
        }
    }

    tracing::info!("Terminal MCP Server stopped");
    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Functions: CLI Commands
//--------------------------------------------------------------------------------------------------

/// List all active sessions.
fn cmd_list() -> io::Result<()> {
    let socket_dir = Path::new(SOCKET_DIR);

    if !socket_dir.exists() {
        println!("No active sessions");
        return Ok(());
    }

    let mut sessions = Vec::new();

    for entry in std::fs::read_dir(socket_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().is_some_and(|e| e == "sock") {
            if let Some(name) = path.file_stem() {
                let session_id = name.to_string_lossy().into_owned();

                // Try to connect and get info
                match get_session_info(&path) {
                    Ok(info) => {
                        sessions.push((session_id, Some(info)));
                    }
                    Err(_) => {
                        // Socket exists but can't connect - stale socket
                        sessions.push((session_id, None));
                    }
                }
            }
        }
    }

    if sessions.is_empty() {
        println!("No active sessions");
        return Ok(());
    }

    // Print header
    println!(
        "{:<12} {:<20} {:<8} {:<10}",
        "ID", "PROGRAM", "PID", "SIZE"
    );
    println!("{}", "-".repeat(52));

    let session_count = sessions.len();
    for (id, info) in sessions {
        match info {
            Some(info) => {
                let short_id = if id.len() > 10 { &id[..10] } else { &id };
                let program = Path::new(&info.program)
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or(info.program);
                let pid = info
                    .pid
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "-".into());
                let size = format!("{}x{}", info.dimensions.cols, info.dimensions.rows);

                println!("{:<12} {:<20} {:<8} {:<10}", short_id, program, pid, size);
            }
            None => {
                let short_id = if id.len() > 10 { &id[..10] } else { &id };
                println!("{:<12} {:<20} {:<8} {:<10}", short_id, "(stale)", "-", "-");
            }
        }
    }

    println!();
    println!("{} session(s)", session_count);

    Ok(())
}

/// Get session info by connecting to the socket.
fn get_session_info(socket_path: &Path) -> io::Result<SessionInfoPayload> {
    let mut stream = UnixStream::connect(socket_path)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;

    // Read the info message
    let mut header = [0u8; HEADER_SIZE];
    stream.read_exact(&mut header)?;

    let msg_type = header[0];
    let len = u32::from_be_bytes([header[1], header[2], header[3], header[4]]) as usize;

    if msg_type != MSG_INFO {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Expected INFO message",
        ));
    }

    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload)?;

    let info: SessionInfoPayload = serde_json::from_slice(&payload)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    Ok(info)
}

/// Show detailed info about a session.
fn cmd_info(session_id: &str) -> io::Result<()> {
    let socket_path = find_session_socket(session_id)?;
    let info = get_session_info(&socket_path)?;

    println!("Session ID:  {}", info.session_id);
    println!("Program:     {}", info.program);
    if !info.args.is_empty() {
        println!("Arguments:   {}", info.args.join(" "));
    }
    if let Some(pid) = info.pid {
        println!("PID:         {}", pid);
    }
    println!(
        "Dimensions:  {}x{}",
        info.dimensions.cols, info.dimensions.rows
    );
    println!("Socket:      {}", socket_path.display());

    Ok(())
}

/// Find the socket path for a session ID (supports prefix matching).
fn find_session_socket(session_id: &str) -> io::Result<std::path::PathBuf> {
    let socket_dir = Path::new(SOCKET_DIR);

    if !socket_dir.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "No active sessions",
        ));
    }

    // First try exact match
    let exact_path = socket_dir.join(format!("{}.sock", session_id));
    if exact_path.exists() {
        return Ok(exact_path);
    }

    // Try prefix match
    let mut matches = Vec::new();
    for entry in std::fs::read_dir(socket_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().is_some_and(|e| e == "sock") {
            if let Some(name) = path.file_stem() {
                let name_str = name.to_string_lossy();
                if name_str.starts_with(session_id) {
                    matches.push(path);
                }
            }
        }
    }

    match matches.len() {
        0 => Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("No session found matching '{}'", session_id),
        )),
        1 => Ok(matches.remove(0)),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Ambiguous session ID '{}', matches {} sessions",
                session_id,
                matches.len()
            ),
        )),
    }
}

/// Attach to a session.
fn cmd_attach(session_id: &str) -> io::Result<()> {
    let socket_path = find_session_socket(session_id)?;

    // Connect to socket
    let stream = UnixStream::connect(&socket_path)?;
    stream.set_nonblocking(true)?;

    // Read initial info
    let mut stream_blocking = stream.try_clone()?;
    stream_blocking.set_nonblocking(false)?;
    stream_blocking.set_read_timeout(Some(Duration::from_secs(5)))?;

    let mut header = [0u8; HEADER_SIZE];
    stream_blocking.read_exact(&mut header)?;

    let msg_type = header[0];
    let len = u32::from_be_bytes([header[1], header[2], header[3], header[4]]) as usize;

    if msg_type != MSG_INFO {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Expected INFO message",
        ));
    }

    let mut payload = vec![0u8; len];
    stream_blocking.read_exact(&mut payload)?;

    let info: SessionInfoPayload = serde_json::from_slice(&payload)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    drop(stream_blocking);

    // Setup terminal
    let mut stdout = io::stdout();
    terminal::enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen)?;

    // Print initial screen content
    print!("{}", info.screen);
    stdout.flush()?;

    // Setup shutdown flag
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    // Handle Ctrl+C
    ctrlc::set_handler(move || {
        running_clone.store(false, Ordering::SeqCst);
    })
    .ok();

    // Clone stream for reading
    let mut read_stream = stream.try_clone()?;
    let mut write_stream = stream;

    // Main loop
    let result = run_attach_loop(&mut read_stream, &mut write_stream, &running, &mut stdout);

    // Cleanup terminal
    terminal::disable_raw_mode()?;
    execute!(stdout, LeaveAlternateScreen)?;

    result
}

/// Main attach loop - handles input and output.
fn run_attach_loop(
    read_stream: &mut UnixStream,
    write_stream: &mut UnixStream,
    running: &AtomicBool,
    stdout: &mut io::Stdout,
) -> io::Result<()> {
    let mut read_buf = [0u8; 4096];

    while running.load(Ordering::SeqCst) {
        // Check for input events (non-blocking)
        if event::poll(Duration::from_millis(10))? {
            match event::read()? {
                Event::Key(key_event) => {
                    // Check for Ctrl+C to detach
                    if key_event.code == KeyCode::Char('c')
                        && key_event.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        break;
                    }

                    // Convert key event to bytes
                    if let Some(bytes) = key_to_bytes(&key_event) {
                        send_input(write_stream, &bytes)?;
                    }
                }
                Event::Resize(cols, rows) => {
                    send_resize(write_stream, rows, cols)?;
                }
                _ => {}
            }
        }

        // Try to read output from socket
        match read_stream.read(&mut read_buf) {
            Ok(0) => {
                // Connection closed
                break;
            }
            Ok(n) => {
                // Parse and handle messages
                let mut pos = 0;
                while pos + HEADER_SIZE <= n {
                    let msg_type = read_buf[pos];
                    let len = u32::from_be_bytes([
                        read_buf[pos + 1],
                        read_buf[pos + 2],
                        read_buf[pos + 3],
                        read_buf[pos + 4],
                    ]) as usize;

                    pos += HEADER_SIZE;

                    if pos + len > n {
                        // Incomplete message, would need buffering
                        break;
                    }

                    match msg_type {
                        MSG_OUTPUT => {
                            // Write output to terminal
                            stdout.write_all(&read_buf[pos..pos + len])?;
                            stdout.flush()?;
                        }
                        MSG_CLOSE => {
                            // Session closed
                            running.store(false, Ordering::SeqCst);
                            break;
                        }
                        _ => {
                            // Ignore other message types
                        }
                    }

                    pos += len;
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                // No data available, continue
            }
            Err(e) => {
                return Err(e);
            }
        }
    }

    Ok(())
}

/// Send input bytes to the session.
fn send_input(stream: &mut UnixStream, data: &[u8]) -> io::Result<()> {
    let len = data.len() as u32;
    let mut msg = Vec::with_capacity(HEADER_SIZE + data.len());
    msg.push(MSG_INPUT);
    msg.extend_from_slice(&len.to_be_bytes());
    msg.extend_from_slice(data);

    stream.set_nonblocking(false)?;
    stream.write_all(&msg)?;
    stream.flush()?;
    stream.set_nonblocking(true)?;

    Ok(())
}

/// Send resize message.
fn send_resize(stream: &mut UnixStream, rows: u16, cols: u16) -> io::Result<()> {
    let mut msg = Vec::with_capacity(HEADER_SIZE + 4);
    msg.push(MSG_RESIZE);
    msg.extend_from_slice(&4u32.to_be_bytes());
    msg.extend_from_slice(&rows.to_be_bytes());
    msg.extend_from_slice(&cols.to_be_bytes());

    stream.set_nonblocking(false)?;
    stream.write_all(&msg)?;
    stream.flush()?;
    stream.set_nonblocking(true)?;

    Ok(())
}

/// Convert a key event to bytes to send to PTY.
fn key_to_bytes(key: &event::KeyEvent) -> Option<Vec<u8>> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);

    let bytes = match key.code {
        KeyCode::Char(c) => {
            if ctrl && c.is_ascii_lowercase() {
                // Ctrl+letter -> control character
                vec![(c as u8) - b'a' + 1]
            } else if alt {
                // Alt+key -> ESC + key
                let mut v = vec![0x1b];
                let mut buf = [0u8; 4];
                v.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                v
            } else {
                let mut buf = [0u8; 4];
                c.encode_utf8(&mut buf).as_bytes().to_vec()
            }
        }
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => vec![0x1b, b'[', b'A'],
        KeyCode::Down => vec![0x1b, b'[', b'B'],
        KeyCode::Right => vec![0x1b, b'[', b'C'],
        KeyCode::Left => vec![0x1b, b'[', b'D'],
        KeyCode::Home => vec![0x1b, b'[', b'H'],
        KeyCode::End => vec![0x1b, b'[', b'F'],
        KeyCode::PageUp => vec![0x1b, b'[', b'5', b'~'],
        KeyCode::PageDown => vec![0x1b, b'[', b'6', b'~'],
        KeyCode::Delete => vec![0x1b, b'[', b'3', b'~'],
        KeyCode::Insert => vec![0x1b, b'[', b'2', b'~'],
        KeyCode::F(n) => match n {
            1 => vec![0x1b, b'O', b'P'],
            2 => vec![0x1b, b'O', b'Q'],
            3 => vec![0x1b, b'O', b'R'],
            4 => vec![0x1b, b'O', b'S'],
            5 => vec![0x1b, b'[', b'1', b'5', b'~'],
            6 => vec![0x1b, b'[', b'1', b'7', b'~'],
            7 => vec![0x1b, b'[', b'1', b'8', b'~'],
            8 => vec![0x1b, b'[', b'1', b'9', b'~'],
            9 => vec![0x1b, b'[', b'2', b'0', b'~'],
            10 => vec![0x1b, b'[', b'2', b'1', b'~'],
            11 => vec![0x1b, b'[', b'2', b'3', b'~'],
            12 => vec![0x1b, b'[', b'2', b'4', b'~'],
            _ => return None,
        },
        _ => return None,
    };

    Some(bytes)
}
