//! caffeine-core: framework-free Win32 core for Caffeine Desktop.
//!
//! The Flutter GUI talks to this crate (via flutter_rust_bridge). All window
//! handles here are raw `isize` HWNDs and every Win32 signature is declared
//! explicitly, so there is zero crates.io API drift for the native surface.
//! The `windows` crate is used only for power-state, last-input, hooks and
//! shell shortcuts.

pub mod api;
pub mod autostart;
pub mod awake;
pub mod cursor_find;
#[allow(clippy::all)]
pub mod frb_generated;
pub mod idle;
pub mod log;
pub mod manager;
pub mod osd;
pub mod overlay;
pub mod settings;
pub mod unlock;
pub mod win32;
