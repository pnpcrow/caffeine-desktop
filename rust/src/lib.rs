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

#[cfg(test)]
pub(crate) mod testsupport {
    use std::sync::{Mutex, MutexGuard, OnceLock};

    /// Serializes tests that create real Win32 windows or share the global
    /// OSD/blackout state: cargo test runs them on parallel threads inside
    /// one process, and FindWindow/state cells are process-global.
    pub(crate) fn window_test_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(|e| e.into_inner())
    }
}
