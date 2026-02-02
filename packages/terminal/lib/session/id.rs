//! Session ID generation.

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Generate a unique session ID.
///
/// Format: "sess_" + 8 random alphanumeric characters.
/// Example: "sess_a1b2c3d4"
pub fn generate_session_id() -> String {
    let suffix: String = uuid::Uuid::new_v4()
        .to_string()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .take(8)
        .collect();
    format!("sess_{}", suffix)
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_session_id_format() {
        let id = generate_session_id();
        assert!(id.starts_with("sess_"));
        assert_eq!(id.len(), 13); // "sess_" (5) + 8 chars
    }

    #[test]
    fn test_session_id_uniqueness() {
        let mut ids = HashSet::new();
        for _ in 0..1000 {
            let id = generate_session_id();
            assert!(ids.insert(id), "Duplicate session ID generated");
        }
    }

    #[test]
    fn test_session_id_alphanumeric() {
        let id = generate_session_id();
        let suffix = &id[5..];
        assert!(suffix.chars().all(|c| c.is_alphanumeric()));
    }
}
