//! Shell prompt detection.

use regex::Regex;

use crate::types::Result;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Detects shell prompts using configurable patterns.
#[derive(Debug)]
pub struct PromptDetector {
    pattern: Regex,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl PromptDetector {
    /// Create a new prompt detector with the given pattern.
    pub fn new(pattern: &str) -> Result<Self> {
        let regex = Regex::new(pattern)?;
        Ok(Self { pattern: regex })
    }

    /// Create a detector with the default pattern.
    pub fn default_pattern() -> Self {
        // Matches common shell prompts: $ , # , >
        Self::new(r"\$\s*$|#\s*$|>\s*$").expect("Default pattern is valid")
    }

    /// Check if content ends with a shell prompt.
    pub fn detect(&self, content: &str) -> bool {
        // Check the last few lines for a prompt
        let lines: Vec<&str> = content.lines().collect();

        // Check last line and second-to-last (in case of trailing newline)
        for line in lines.iter().rev().take(2) {
            let trimmed = line.trim_end();
            if !trimmed.is_empty() && self.pattern.is_match(trimmed) {
                return true;
            }
        }

        false
    }

    /// Get the pattern string.
    pub fn pattern(&self) -> &str {
        self.pattern.as_str()
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl Default for PromptDetector {
    fn default() -> Self {
        Self::default_pattern()
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bash_prompt() {
        let detector = PromptDetector::default();
        assert!(detector.detect("user@host:~$ "));
        assert!(detector.detect("$ "));
        assert!(detector.detect("some output\n$ "));
    }

    #[test]
    fn test_root_prompt() {
        let detector = PromptDetector::default();
        assert!(detector.detect("root@host:~# "));
        assert!(detector.detect("# "));
    }

    #[test]
    fn test_zsh_prompt() {
        let detector = PromptDetector::default();
        // % is not in default pattern
        assert!(!detector.detect("% "));

        // But > is
        assert!(detector.detect("> "));
    }

    #[test]
    fn test_no_prompt() {
        let detector = PromptDetector::default();
        assert!(!detector.detect("Still running..."));
        assert!(!detector.detect(""));
        assert!(!detector.detect("some output without prompt"));
    }

    #[test]
    fn test_prompt_in_output() {
        let detector = PromptDetector::default();
        // Prompt should be at the end
        assert!(!detector.detect("$ echo hello\nhello"));
        assert!(detector.detect("$ echo hello\nhello\n$ "));
    }

    #[test]
    fn test_custom_pattern() {
        let detector = PromptDetector::new(r">>>").unwrap();
        assert!(detector.detect(">>> ")); // Python REPL
        assert!(!detector.detect("$ "));
    }

    #[test]
    fn test_trailing_newline() {
        let detector = PromptDetector::default();
        assert!(detector.detect("output\n$ \n"));
    }
}
