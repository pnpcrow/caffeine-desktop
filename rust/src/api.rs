//! flutter_rust_bridge boundary: the only functions Dart may call.
//!
//! All functions are synchronous and non-blocking; FRB runs them on its own
//! worker pool, so the Flutter UI thread can never be stalled by the core.
//! State changes flow back push-style via `watch_events`.

use crate::frb_generated::StreamSink;
use crate::manager::{self, Status};
use crate::settings::Settings;

pub fn init_core() {
    manager::init_core();
}

/// Subscribe to status changes. The current status is pushed immediately.
pub fn watch_events(sink: StreamSink<Status>) {
    manager::watch_events(sink);
}

pub fn get_status() -> Status {
    manager::core().status()
}

pub fn get_settings() -> Settings {
    manager::core().settings_snapshot()
}

pub fn save_settings(s: Settings) -> Result<(), String> {
    let awake = s.awake_enabled;
    manager::core().update_settings(s.clone());
    manager::core().apply_awake(awake);
    s.save()?;
    manager::core().emit();
    Ok(())
}

pub fn set_awake(on: bool) {
    let c = manager::core();
    c.set_awake_flag(on);
    c.emit();
}

pub fn set_auto_blackout(on: bool) {
    let c = manager::core();
    c.set_auto_flag(on);
    c.emit();
}

pub fn blackout_now() {
    let c = manager::core();
    c.overlay_show();
    c.emit();
}

pub fn clear_blackout() {
    let c = manager::core();
    c.overlay_hide();
    c.emit();
}

/// Restore the power state before process exit (Dart calls `exit()` after).
pub fn prepare_quit() {
    crate::awake::apply_awake(false);
}

/// Process singleton. Returns true for the first owner; later instances
/// should focus the existing settings window and exit.
pub fn ensure_single_instance() -> bool {
    crate::win32::take_single_instance("CaffeineDesktopSingleton-v1")
}

/// Bring the first instance's settings window forward. Best-effort.
pub fn focus_existing_window() -> bool {
    crate::win32::focus_window_by_title("Caffeine Desktop")
}
