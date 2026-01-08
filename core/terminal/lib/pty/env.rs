//! Environment variable filtering for spawned processes.

use std::collections::HashMap;

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Build environment for spawned process, filtering sensitive variables.
pub fn build_environment(extra: &HashMap<String, String>, term: &str) -> HashMap<String, String> {
    let mut env: HashMap<String, String> = std::env::vars()
        .filter(|(k, _)| !is_sensitive_var(k))
        .collect();

    // Set TERM
    env.insert("TERM".to_string(), term.to_string());

    // Add user-provided vars (can override)
    env.extend(extra.clone());

    env
}

/// Check if an environment variable name is sensitive and should be filtered.
fn is_sensitive_var(name: &str) -> bool {
    // Explicit sensitive variables
    let explicit_sensitive = matches!(
        name,
        "SSH_AUTH_SOCK"
            | "SSH_AGENT_PID"
            | "GPG_AGENT_INFO"
            | "AWS_SECRET_ACCESS_KEY"
            | "AWS_SESSION_TOKEN"
            | "GITHUB_TOKEN"
            | "ANTHROPIC_API_KEY"
            | "OPENAI_API_KEY"
            | "CLAUDE_API_KEY"
            | "HF_TOKEN"
            | "HUGGINGFACE_TOKEN"
    );

    if explicit_sensitive {
        return true;
    }

    // Pattern-based filtering
    let name_upper = name.to_uppercase();
    name_upper.contains("SECRET")
        || name_upper.contains("PASSWORD")
        || name_upper.contains("CREDENTIAL")
        || name_upper.contains("PRIVATE_KEY")
        || (name_upper.contains("API") && name_upper.contains("KEY"))
        || (name_upper.contains("AUTH") && name_upper.contains("TOKEN"))
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sensitive_vars_filtered() {
        assert!(is_sensitive_var("AWS_SECRET_ACCESS_KEY"));
        assert!(is_sensitive_var("GITHUB_TOKEN"));
        assert!(is_sensitive_var("MY_SECRET_VALUE"));
        assert!(is_sensitive_var("DATABASE_PASSWORD"));
        assert!(is_sensitive_var("PRIVATE_KEY_PATH"));
        assert!(is_sensitive_var("MY_API_KEY"));
    }

    #[test]
    fn test_safe_vars_allowed() {
        assert!(!is_sensitive_var("HOME"));
        assert!(!is_sensitive_var("PATH"));
        assert!(!is_sensitive_var("USER"));
        assert!(!is_sensitive_var("SHELL"));
        assert!(!is_sensitive_var("TERM"));
        assert!(!is_sensitive_var("LANG"));
    }

    #[test]
    fn test_term_set() {
        let env = build_environment(&HashMap::new(), "xterm-256color");
        assert_eq!(env.get("TERM"), Some(&"xterm-256color".to_string()));
    }

    #[test]
    fn test_extra_vars_added() {
        let mut extra = HashMap::new();
        extra.insert("MY_VAR".to_string(), "my_value".to_string());

        let env = build_environment(&extra, "xterm");
        assert_eq!(env.get("MY_VAR"), Some(&"my_value".to_string()));
    }

    #[test]
    fn test_extra_vars_override() {
        let mut extra = HashMap::new();
        extra.insert("TERM".to_string(), "custom-term".to_string());

        let env = build_environment(&extra, "xterm");
        assert_eq!(env.get("TERM"), Some(&"custom-term".to_string()));
    }
}
