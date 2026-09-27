pub mod headless;

#[cfg(feature = "tui")]
pub mod ratatui;

#[cfg(feature = "tui")]
pub mod text;

#[cfg(feature = "graphics")]
pub mod framebuffer;
