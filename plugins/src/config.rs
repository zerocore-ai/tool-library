//! User configuration for the plugins MCP server.

use std::sync::OnceLock;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// Default registry URL.
pub const DEFAULT_REGISTRY_URL: &str = "https://plugin.store";

/// Environment variable for registry URL.
pub const REGISTRY_URL_ENV: &str = "REGISTRY_URL";

/// Environment variable for registry fallback setting.
pub const USE_REGISTRY_FALLBACK_ENV: &str = "USE_REGISTRY_FALLBACK";

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// User configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    /// Base URL of the plugin registry.
    pub registry_url: String,

    /// Whether to fall back to the registry when plugins are not found locally.
    pub use_registry_fallback: bool,
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl Default for Config {
    fn default() -> Self {
        Self {
            registry_url: DEFAULT_REGISTRY_URL.to_string(),
            use_registry_fallback: true,
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl Config {
    /// Load configuration from environment variables.
    pub fn from_env() -> Self {
        let registry_url = std::env::var(REGISTRY_URL_ENV)
            .unwrap_or_else(|_| DEFAULT_REGISTRY_URL.to_string());

        let use_registry_fallback = std::env::var(USE_REGISTRY_FALLBACK_ENV)
            .map(|v| !matches!(v.to_lowercase().as_str(), "false" | "0" | "no"))
            .unwrap_or(true);

        Self {
            registry_url,
            use_registry_fallback,
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Global configuration singleton.
static CONFIG: OnceLock<Config> = OnceLock::new();

/// Get the global configuration.
pub fn config() -> &'static Config {
    CONFIG.get_or_init(Config::from_env)
}
