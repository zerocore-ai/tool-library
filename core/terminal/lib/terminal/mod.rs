//! Terminal emulation components.

mod ansi;
mod cursor;
mod emulator;
mod prompt;
mod screen;
mod scrollback;
mod state;
mod tracker;

pub use ansi::strip_ansi;
pub use cursor::CursorState;
pub use emulator::ScreenPerformer;
pub use prompt::PromptDetector;
pub use screen::{Cell, CellAttributes, Color, ScreenBuffer, ScrollbackLine};
pub use scrollback::ScrollbackBuffer;
pub use state::TerminalState;
pub use tracker::OutputTracker;
