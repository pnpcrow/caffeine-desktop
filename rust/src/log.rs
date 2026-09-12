//! Dual-sink diagnostic log: stderr (debug console) + rotating session file.
//!
//! Release GUI builds have no console, so Rust diagnostics would otherwise
//! be invisible. The file lives at
//! `%LOCALAPPDATA%/caffeine-desktop/core.log` (truncated per session).

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

static FILE: OnceLock<Mutex<std::fs::File>> = OnceLock::new();

fn stamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Open (truncate) the session log. Called once from `init_core`.
pub fn init_file_log() {
    let path = std::env::var("LOCALAPPDATA")
        .map(|b| {
            std::path::PathBuf::from(b)
                .join("caffeine-desktop")
                .join("core.log")
        })
        .unwrap_or_else(|_| std::path::PathBuf::from("caffeine-core.log"));
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
    {
        Ok(f) => {
            let _ = FILE.set(Mutex::new(f));
            log_line("core log opened");
        }
        Err(e) => {
            eprintln!("[caffeine] log file unavailable: {e}");
        }
    }
}

pub fn log_line(msg: &str) {
    eprintln!("[caffeine] {msg}");
    if let Some(f) = FILE.get() {
        if let Ok(mut f) = f.lock() {
            let _ = writeln!(f, "[{}] {msg}", stamp());
        }
    }
}
