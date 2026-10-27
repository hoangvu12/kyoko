pub mod autostart;
pub mod caret;
pub mod clipboard;
pub mod foreground;
pub mod hotkeys;
pub mod idle;
pub mod instance;
pub mod keyboard;
pub mod notifications;
pub mod open;
pub mod relaunch;
pub mod system_theme;
pub mod tray;
pub mod url_scheme;

mod error;
mod registry;

pub use error::{PlatformError, Result};
