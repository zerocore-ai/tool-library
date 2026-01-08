//! Background PTY reader thread.

use std::io::{ErrorKind, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use tokio::sync::mpsc;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Messages sent from the reader thread.
#[derive(Debug)]
pub enum ReaderMessage {
    /// Raw PTY output data.
    Data(Vec<u8>),

    /// Process exited with optional exit code.
    Exited(Option<i32>),

    /// Fatal read error occurred.
    Error(String),

    /// End of file (PTY closed).
    Eof,
}

/// Background reader that continuously reads PTY output.
pub struct SessionReader {
    handle: Option<JoinHandle<()>>,
    rx: mpsc::Receiver<ReaderMessage>,
    shutdown: Arc<AtomicBool>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl SessionReader {
    /// Spawn a reader thread for the given PTY reader.
    pub fn spawn(mut pty_reader: Box<dyn Read + Send>) -> Self {
        let (tx, rx) = mpsc::channel::<ReaderMessage>(1024);
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();

        let handle = std::thread::spawn(move || {
            let mut buf = [0u8; 4096];

            loop {
                if shutdown_clone.load(Ordering::Relaxed) {
                    break;
                }

                match pty_reader.read(&mut buf) {
                    Ok(0) => {
                        // EOF - PTY closed
                        let _ = tx.blocking_send(ReaderMessage::Eof);
                        break;
                    }
                    Ok(n) => {
                        if tx.blocking_send(ReaderMessage::Data(buf[..n].to_vec())).is_err() {
                            // Receiver dropped
                            break;
                        }
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => {
                        // No data available, sleep briefly
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) if e.kind() == ErrorKind::Interrupted => {
                        // Interrupted, retry
                        continue;
                    }
                    Err(e) => {
                        let _ = tx.blocking_send(ReaderMessage::Error(e.to_string()));
                        break;
                    }
                }
            }
        });

        Self {
            handle: Some(handle),
            rx,
            shutdown,
        }
    }

    /// Try to receive a message without blocking.
    pub fn try_recv(&mut self) -> Option<ReaderMessage> {
        self.rx.try_recv().ok()
    }

    /// Receive a message, waiting up to the specified duration.
    pub async fn recv_timeout(&mut self, timeout: Duration) -> Option<ReaderMessage> {
        tokio::time::timeout(timeout, self.rx.recv())
            .await
            .ok()
            .flatten()
    }

    /// Drain all available messages.
    pub fn drain(&mut self) -> Vec<ReaderMessage> {
        let mut messages = Vec::new();
        while let Some(msg) = self.try_recv() {
            messages.push(msg);
        }
        messages
    }

    /// Check if there are pending messages without consuming them.
    pub fn has_pending(&self) -> bool {
        !self.rx.is_empty()
    }

    /// Signal shutdown to the reader thread.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }

    /// Check if the reader thread has finished.
    pub fn is_finished(&self) -> bool {
        self.handle.as_ref().is_some_and(|h| h.is_finished())
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl Drop for SessionReader {
    fn drop(&mut self) {
        self.shutdown();
        if let Some(handle) = self.handle.take() {
            // Give the thread a short time to exit gracefully
            // If it doesn't exit in time, we detach it (it will exit when the PTY closes)
            let start = std::time::Instant::now();
            while !handle.is_finished() {
                if start.elapsed() > Duration::from_millis(100) {
                    // Thread didn't exit in time - detach and let it die with the PTY
                    tracing::debug!("Reader thread didn't exit in time, detaching");
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            if handle.is_finished() {
                let _ = handle.join();
            }
        }
    }
}

impl std::fmt::Debug for SessionReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionReader")
            .field("shutdown", &self.shutdown.load(Ordering::Relaxed))
            .field("has_pending", &self.has_pending())
            .finish()
    }
}
