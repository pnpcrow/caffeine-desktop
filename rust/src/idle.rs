//! Idle-time detection via GetLastInputInfo.
//!
//! Note: synthetic input from an AI Computer-Use agent (SendInput etc.)
//! updates the last-input timestamp like physical input, so auto blackout
//! naturally stays out of the way while the agent is working.

use crate::win32::tick_ms;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

/// Milliseconds since the last keyboard/mouse input (any session input).
pub fn idle_ms() -> u64 {
    unsafe {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if GetLastInputInfo(&mut info).as_bool() {
            tick_ms().wrapping_sub(info.dwTime) as u64
        } else {
            0
        }
    }
}
