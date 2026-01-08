//! MCP tool implementations for the plugins server.

mod resolve;
mod search;

pub use resolve::{handle_resolve, ResolveInput, ResolveOutput};
pub use search::{handle_search, SearchInput, SearchOutput};
