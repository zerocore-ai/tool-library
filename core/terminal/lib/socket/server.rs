//! Unix socket server for session attachment.
//!
//! Each terminal session can expose a Unix socket that allows external clients
//! to attach and interact with the session in real-time.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{broadcast, mpsc, Mutex};

use super::protocol::{write_message, Message, ProtocolError, SessionInfoPayload};
use crate::types::Dimensions;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// Default socket directory.
pub const SOCKET_DIR: &str = "/tmp/terminal";

/// Maximum number of clients per session.
const MAX_CLIENTS: usize = 10;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Handle to a running socket server for a session.
pub struct SocketServer {
    /// Session ID.
    session_id: String,

    /// Path to the socket file.
    socket_path: PathBuf,

    /// Channel to send input from clients to the session.
    input_tx: mpsc::Sender<SocketInput>,

    /// Channel to broadcast output to all connected clients.
    output_tx: broadcast::Sender<Vec<u8>>,

    /// Shutdown signal.
    shutdown_tx: mpsc::Sender<()>,

    /// Server task handle.
    handle: Option<tokio::task::JoinHandle<()>>,
}

/// Input received from a socket client.
#[derive(Debug, Clone)]
pub enum SocketInput {
    /// Raw input bytes to send to PTY.
    Data(Vec<u8>),

    /// Resize request.
    Resize { rows: u16, cols: u16 },
}

/// State shared between the server and client handlers.
struct ServerState {
    session_id: String,
    program: String,
    args: Vec<String>,
    pid: Option<u32>,
    dimensions: Mutex<Dimensions>,
    screen_fn: Box<dyn Fn() -> String + Send + Sync>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl SocketServer {
    /// Create and start a socket server for a session.
    pub fn start(
        session_id: String,
        program: String,
        args: Vec<String>,
        pid: Option<u32>,
        dimensions: Dimensions,
        screen_fn: impl Fn() -> String + Send + Sync + 'static,
    ) -> std::io::Result<(Self, mpsc::Receiver<SocketInput>)> {
        // Ensure socket directory exists
        let socket_dir = Path::new(SOCKET_DIR);
        if !socket_dir.exists() {
            std::fs::create_dir_all(socket_dir)?;
        }

        let socket_path = socket_dir.join(format!("{}.sock", session_id));

        // Remove stale socket if exists
        if socket_path.exists() {
            std::fs::remove_file(&socket_path)?;
        }

        // Create channels
        let (input_tx, input_rx) = mpsc::channel::<SocketInput>(256);
        let (output_tx, _) = broadcast::channel::<Vec<u8>>(1024);
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

        // Bind the socket
        let listener = std::os::unix::net::UnixListener::bind(&socket_path)?;
        listener.set_nonblocking(true)?;

        // Convert to tokio listener
        let listener = UnixListener::from_std(listener)?;

        let state = Arc::new(ServerState {
            session_id: session_id.clone(),
            program,
            args,
            pid,
            dimensions: Mutex::new(dimensions),
            screen_fn: Box::new(screen_fn),
        });

        let output_tx_clone = output_tx.clone();
        let input_tx_clone = input_tx.clone();
        let socket_path_clone = socket_path.clone();

        // Spawn the server task
        let handle = tokio::spawn(async move {
            tracing::debug!(path = %socket_path_clone.display(), "Socket server started");

            loop {
                tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok((stream, _)) => {
                                let client_count = output_tx_clone.receiver_count();
                                if client_count >= MAX_CLIENTS {
                                    tracing::warn!("Max clients reached, rejecting connection");
                                    continue;
                                }

                                tracing::debug!("Client connected (total: {})", client_count + 1);

                                // Spawn client handler
                                let state = state.clone();
                                let input_tx = input_tx_clone.clone();
                                let output_rx = output_tx_clone.subscribe();

                                tokio::spawn(async move {
                                    if let Err(e) = handle_client(stream, state, input_tx, output_rx).await {
                                        match e {
                                            ProtocolError::ConnectionClosed => {
                                                tracing::debug!("Client disconnected");
                                            }
                                            _ => {
                                                tracing::warn!("Client error: {}", e);
                                            }
                                        }
                                    }
                                });
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                // No pending connections, yield
                                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                            }
                            Err(e) => {
                                tracing::error!("Accept error: {}", e);
                                break;
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        tracing::debug!("Socket server shutdown signal received");
                        break;
                    }
                }
            }

            // Cleanup socket file
            let _ = std::fs::remove_file(&socket_path_clone);
            tracing::debug!("Socket server stopped");
        });

        Ok((
            Self {
                session_id,
                socket_path,
                input_tx,
                output_tx,
                shutdown_tx,
                handle: Some(handle),
            },
            input_rx,
        ))
    }

    /// Get the socket path.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Get the session ID.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Broadcast output to all connected clients.
    pub fn broadcast_output(&self, data: &[u8]) {
        // Ignore send errors (no receivers)
        let _ = self.output_tx.send(data.to_vec());
    }

    /// Get the number of connected clients.
    pub fn client_count(&self) -> usize {
        self.output_tx.receiver_count()
    }

    /// Shutdown the socket server.
    pub async fn shutdown(&mut self) {
        // Send shutdown signal
        let _ = self.shutdown_tx.send(()).await;

        // Wait for task to complete
        if let Some(handle) = self.handle.take() {
            let _ = handle.await;
        }

        // Ensure socket file is removed
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

/// Handle a connected client.
async fn handle_client(
    stream: UnixStream,
    state: Arc<ServerState>,
    input_tx: mpsc::Sender<SocketInput>,
    mut output_rx: broadcast::Receiver<Vec<u8>>,
) -> Result<(), ProtocolError> {
    let (mut reader, mut writer) = stream.into_split();

    // Send session info on connect
    let dimensions = *state.dimensions.lock().await;
    let screen = (state.screen_fn)();
    let info = SessionInfoPayload {
        session_id: state.session_id.clone(),
        program: state.program.clone(),
        args: state.args.clone(),
        pid: state.pid,
        dimensions,
        screen,
    };
    write_message(&mut writer, &Message::Info(info)).await?;

    // Spawn output forwarder
    let writer = Arc::new(Mutex::new(writer));
    let writer_clone = writer.clone();

    let output_task = tokio::spawn(async move {
        loop {
            match output_rx.recv().await {
                Ok(data) => {
                    let msg = Message::Output(data);
                    let mut w = writer_clone.lock().await;
                    if let Err(e) = write_message(&mut *w, &msg).await {
                        tracing::debug!("Output write error: {}", e);
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("Client lagged by {} messages", n);
                }
            }
        }
    });

    // Read input from client
    loop {
        match super::protocol::read_message(&mut reader).await {
            Ok(Message::Input(data)) => {
                if input_tx.send(SocketInput::Data(data)).await.is_err() {
                    break;
                }
            }
            Ok(Message::Resize { rows, cols }) => {
                *state.dimensions.lock().await = Dimensions { rows, cols };
                if input_tx
                    .send(SocketInput::Resize { rows, cols })
                    .await
                    .is_err()
                {
                    break;
                }
            }
            Ok(Message::Close(_)) => {
                break;
            }
            Ok(_) => {
                // Ignore unexpected message types
            }
            Err(ProtocolError::ConnectionClosed) => {
                break;
            }
            Err(e) => {
                tracing::warn!("Client read error: {}", e);
                break;
            }
        }
    }

    // Cleanup
    output_task.abort();

    // Send close message
    let mut w = writer.lock().await;
    let _ = write_message(&mut *w, &Message::Close(None)).await;

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl Drop for SocketServer {
    fn drop(&mut self) {
        // Ensure socket file is cleaned up
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// List all active session sockets.
pub fn list_sockets() -> std::io::Result<Vec<String>> {
    let socket_dir = Path::new(SOCKET_DIR);
    if !socket_dir.exists() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();
    for entry in std::fs::read_dir(socket_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "sock") {
            if let Some(name) = path.file_stem() {
                sessions.push(name.to_string_lossy().into_owned());
            }
        }
    }

    Ok(sessions)
}

/// Get the socket path for a session.
pub fn socket_path_for(session_id: &str) -> PathBuf {
    Path::new(SOCKET_DIR).join(format!("{}.sock", session_id))
}
