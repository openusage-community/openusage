// Windows support adapted from barramee27/crossusage (MIT): https://github.com/barramee27/crossusage
//! Ratatui dashboard — btop-style OpenUsage TUI.

mod app;
mod picker;
mod platform;
mod state;
pub mod theme;
pub mod view_model;

pub use app::run;
pub use platform::ignore_sigtstp;
