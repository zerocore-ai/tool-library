//! Integration tests for the Terminal MCP Server.
//!
//! These tests spawn actual PTY sessions and verify the full flow.
//! We use short-lived programs (echo, cat, sleep) to avoid shell timing issues.

use std::collections::HashMap;
use std::time::Duration;

use terminal::{
    GlobalConfig, OutputFormat, SessionManager, ViewMode,
    session::{CreateSessionOptions, is_shell_program},
};

//--------------------------------------------------------------------------------------------------
// Helper Functions
//--------------------------------------------------------------------------------------------------

fn create_test_config() -> GlobalConfig {
    GlobalConfig {
        default_rows: 24,
        default_cols: 80,
        default_shell: "/bin/sh".to_string(),  // sh is lighter than bash
        term: "xterm-256color".to_string(),
        scrollback_limit: 1000,
        prompt_pattern: r"\$\s*$|#\s*$|>\s*$".to_string(),
        max_sessions: 10,
    }
}

/// Create options for a short-lived program (exits immediately)
fn short_lived_opts() -> CreateSessionOptions {
    CreateSessionOptions {
        program: Some("/bin/sleep".to_string()),
        args: vec!["0.05".to_string()],  // 50ms
        ..Default::default()
    }
}

//--------------------------------------------------------------------------------------------------
// Tests: Session Creation
//--------------------------------------------------------------------------------------------------

#[tokio::test]
async fn test_create_session_echo() {
    let config = create_test_config();
    let manager = SessionManager::new(config);

    let opts = CreateSessionOptions {
        program: Some("/bin/echo".to_string()),
        args: vec!["hello".to_string()],
        ..Default::default()
    };

    let result = manager.create_session(opts).await;
    assert!(result.is_ok(), "Failed to create session: {:?}", result.err());

    let info = result.unwrap();
    assert!(!info.session_id.is_empty());
    assert!(info.pid.is_some());
    assert_eq!(info.program, "/bin/echo");

    // Wait for echo to complete
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Cleanup - session should have exited on its own
    manager.shutdown().await;
}

#[tokio::test]
async fn test_create_session_custom_dimensions() {
    let config = create_test_config();
    let manager = SessionManager::new(config);

    let opts = CreateSessionOptions {
        program: Some("/bin/echo".to_string()),
        args: vec!["test".to_string()],
        rows: Some(40),
        cols: Some(120),
        ..Default::default()
    };

    let result = manager.create_session(opts).await;
    assert!(result.is_ok());

    let info = result.unwrap();
    assert_eq!(info.dimensions.rows, 40);
    assert_eq!(info.dimensions.cols, 120);

    tokio::time::sleep(Duration::from_millis(50)).await;
    manager.shutdown().await;
}

#[tokio::test]
async fn test_create_session_with_env() {
    let config = create_test_config();
    let manager = SessionManager::new(config);

    let mut env = HashMap::new();
    env.insert("MY_TEST_VAR".to_string(), "test_value".to_string());

    let opts = CreateSessionOptions {
        program: Some("/bin/sh".to_string()),
        args: vec!["-c".to_string(), "echo $MY_TEST_VAR".to_string()],
        env,
        ..Default::default()
    };

    let result = manager.create_session(opts).await;
    assert!(result.is_ok());

    tokio::time::sleep(Duration::from_millis(100)).await;
    manager.shutdown().await;
}

//--------------------------------------------------------------------------------------------------
// Tests: Session Listing
//--------------------------------------------------------------------------------------------------

#[tokio::test]
async fn test_list_sessions_empty() {
    let config = create_test_config();
    let manager = SessionManager::new(config);

    let sessions = manager.list().await;
    assert!(sessions.is_empty());
}

#[tokio::test]
async fn test_list_sessions_multiple() {
    let config = create_test_config();
    let manager = SessionManager::new(config);

    // Create 3 short-lived sessions
    for _ in 0..3 {
        manager.create_session(short_lived_opts()).await.unwrap();
    }

    let sessions = manager.list().await;
    assert_eq!(sessions.len(), 3);

    // All should have unique IDs
    let ids: Vec<_> = sessions.iter().map(|s| &s.session_id).collect();
    let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(unique_ids.len(), 3);

    // Wait for sleep to complete
    tokio::time::sleep(Duration::from_millis(100)).await;
    manager.shutdown().await;
}

//--------------------------------------------------------------------------------------------------
// Tests: Session Destruction
//--------------------------------------------------------------------------------------------------

#[tokio::test]
async fn test_destroy_session() {
    let config = create_test_config();
    let manager = SessionManager::new(config);

    // Create a cat process (stays alive until we kill it)
    let opts = CreateSessionOptions {
        program: Some("/bin/cat".to_string()),
        args: vec![],
        ..Default::default()
    };

    let info = manager.create_session(opts).await.unwrap();
    let session_id = info.session_id.clone();

    // Verify it exists
    assert!(manager.get(&session_id).await.is_ok());

    // Destroy it (this should kill cat)
    let result = manager.destroy_session(&session_id, true).await;
    assert!(result.is_ok());

    // Verify it's gone
    assert!(manager.get(&session_id).await.is_err());
}

#[tokio::test]
async fn test_destroy_nonexistent_session() {
    let config = create_test_config();
    let manager = SessionManager::new(config);

    let result = manager.destroy_session("nonexistent-id", false).await;
    assert!(result.is_err());
}

//--------------------------------------------------------------------------------------------------
// Tests: Input/Output
//--------------------------------------------------------------------------------------------------

#[tokio::test]
async fn test_send_and_read_echo() {
    let config = create_test_config();
    let manager = SessionManager::new(config);

    // Create a cat process that echoes input
    let opts = CreateSessionOptions {
        program: Some("/bin/cat".to_string()),
        args: vec![],
        ..Default::default()
    };

    let info = manager.create_session(opts).await.unwrap();
    let session_id = info.session_id.clone();

    // Give cat time to start
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Send some text
    {
        let session = manager.get(&session_id).await.unwrap();
        let session = session.lock().await;
        session.state.pty().write(b"hello\n").unwrap();
    }

    // Wait for output
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Read and verify
    {
        let session = manager.get(&session_id).await.unwrap();
        let mut session = session.lock().await;
        session.drain_reader().unwrap();
        let content = session.state.read(ViewMode::Screen, OutputFormat::Plain);
        assert!(content.contains("hello"), "Expected 'hello' in output: {}", content);
    }

    // Force kill cat
    manager.destroy_session(&session_id, true).await.ok();
}

#[tokio::test]
async fn test_read_view_modes() {
    let config = create_test_config();
    let manager = SessionManager::new(config);

    let opts = CreateSessionOptions {
        program: Some("/bin/echo".to_string()),
        args: vec!["test output".to_string()],
        ..Default::default()
    };

    let info = manager.create_session(opts).await.unwrap();
    let session_id = info.session_id.clone();

    // Poll until we get output or timeout
    let mut screen = String::new();
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let session = manager.get(&session_id).await.unwrap();
        let mut session = session.lock().await;
        session.drain_reader().unwrap();
        screen = session.state.read(ViewMode::Screen, OutputFormat::Plain);
        if screen.contains("test output") {
            break;
        }
    }

    // Test screen view
    assert!(screen.contains("test output"), "Expected 'test output' in: '{}'", screen);

    // Test new view
    let session = manager.get(&session_id).await.unwrap();
    let mut session = session.lock().await;

    // New view should have content since tracker wasn't cleared
    let _new_content = session.state.read(ViewMode::New, OutputFormat::Plain);
    // After reading screen, the new content might vary, so just check it doesn't crash

    // Test new view again (should be empty after take)
    let new_again = session.state.read(ViewMode::New, OutputFormat::Plain);
    assert!(new_again.is_empty(), "New view should be empty after take");

    drop(session);
    manager.shutdown().await;
}

//--------------------------------------------------------------------------------------------------
// Tests: Concurrent Access
//--------------------------------------------------------------------------------------------------

#[tokio::test]
async fn test_concurrent_session_access() {
    let config = create_test_config();
    let manager = std::sync::Arc::new(SessionManager::new(config));

    // Use cat so we can control when it exits
    let opts = CreateSessionOptions {
        program: Some("/bin/cat".to_string()),
        args: vec![],
        ..Default::default()
    };

    let info = manager.create_session(opts).await.unwrap();
    let session_id = info.session_id.clone();

    // Spawn multiple tasks accessing the session
    let mut handles = vec![];
    for _ in 0..5 {
        let m = manager.clone();
        let id = session_id.clone();
        let handle = tokio::spawn(async move {
            for _ in 0..10 {
                let session = m.get(&id).await.unwrap();
                let session = session.lock().await;
                let _ = session.info();
                drop(session);
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    // Force kill
    manager.destroy_session(&session_id, true).await.ok();
}

//--------------------------------------------------------------------------------------------------
// Tests: Max Sessions Limit
//--------------------------------------------------------------------------------------------------

#[tokio::test]
async fn test_max_sessions_limit() {
    let mut config = create_test_config();
    config.max_sessions = 2;
    let manager = SessionManager::new(config);

    // Create 2 sessions (should succeed)
    manager.create_session(short_lived_opts()).await.unwrap();
    manager.create_session(short_lived_opts()).await.unwrap();

    // Third should fail
    let result = manager.create_session(short_lived_opts()).await;
    assert!(result.is_err());

    // Wait for sleep to complete
    tokio::time::sleep(Duration::from_millis(100)).await;
    manager.shutdown().await;
}

//--------------------------------------------------------------------------------------------------
// Tests: Helper Functions
//--------------------------------------------------------------------------------------------------

#[test]
fn test_is_shell_program() {
    assert!(is_shell_program("/bin/bash"));
    assert!(is_shell_program("/usr/bin/zsh"));
    assert!(is_shell_program("bash"));
    assert!(is_shell_program("zsh"));
    assert!(is_shell_program("fish"));
    assert!(is_shell_program("sh"));

    assert!(!is_shell_program("/bin/cat"));
    assert!(!is_shell_program("vim"));
    assert!(!is_shell_program("python"));
}

//--------------------------------------------------------------------------------------------------
// Tests: Graceful Shutdown
//--------------------------------------------------------------------------------------------------

#[tokio::test]
async fn test_shutdown_terminates_all_sessions() {
    let config = create_test_config();
    let manager = SessionManager::new(config);

    // Create multiple short-lived sessions
    for _ in 0..3 {
        manager.create_session(short_lived_opts()).await.unwrap();
    }

    // All should exist
    assert_eq!(manager.list().await.len(), 3);

    // Wait a bit for sleep processes
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Shutdown
    manager.shutdown().await;

    // All should be gone
    assert_eq!(manager.list().await.len(), 0);
}

//--------------------------------------------------------------------------------------------------
// Tests: Exit Detection
//--------------------------------------------------------------------------------------------------

#[tokio::test]
async fn test_exit_detection() {
    let config = create_test_config();
    let manager = SessionManager::new(config);

    let opts = CreateSessionOptions {
        program: Some("/bin/sh".to_string()),
        args: vec!["-c".to_string(), "exit 42".to_string()],
        ..Default::default()
    };

    let info = manager.create_session(opts).await.unwrap();
    let session_id = info.session_id;

    // Wait for the command to complete
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Drain reader to process exit
    {
        let session = manager.get(&session_id).await.unwrap();
        let mut session = session.lock().await;
        session.drain_reader().unwrap();

        // Check exit status
        assert!(session.state.exited());
        assert_eq!(session.state.exit_code(), Some(42));
    }

    manager.shutdown().await;
}
