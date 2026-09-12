//! Global low-level keyboard/mouse hooks + shake-to-unlock detection.
//!
//! The blackout overlay itself is click-through (WS_EX_TRANSPARENT), so the
//! AI Computer-Use agent's input always reaches the apps underneath. These
//! hooks only *observe* input to decide when a human wants the overlay gone.
//!
//! AI-friendliness: prefer `UnlockMouse::Shake` (or `Off`) while an agent is
//! driving the pointer — straight synthetic mouse paths do not match a human
//! shake pattern, whereas `Move`/`Click` would dismiss the overlay on the
//! agent's first action.
//!
//! Framework-free: raw input events flow through an `mpsc` channel; the host
//! (Flutter via FRB, or tests) owns the consumer loop.

use crate::settings::{UnlockKey, UnlockMouse};
use std::collections::VecDeque;
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::{HMODULE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK,
    KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL,
};

// --- tunables ---------------------------------------------------------------

/// Mouse trail window for shake detection.
pub const SHAKE_WINDOW_MS: u128 = 2000;
/// Minimum horizontal excursion (px) for a segment to count as a stroke.
pub const SHAKE_MIN_DX: i32 = 25;
/// Direction reversals required for a shake.
pub const SHAKE_FLIPS_NEEDED: u32 = 3;
/// Minimum total pointer path (px) inside the window for a shake.
pub const SHAKE_MIN_PATH: i64 = 450;
/// Pixel distance from the blackout-start anchor that counts as "move".
pub const MOVE_UNLOCK_PX: i32 = 60;

const HC_ACTION: i32 = 0;
const WM_KEYDOWN: u32 = 256;
const WM_SYSKEYDOWN: u32 = 260;
const WM_MOUSEMOVE: u32 = 512;
const WM_LBUTTONDOWN: u32 = 513;
const WM_RBUTTONDOWN: u32 = 516;
const WM_MBUTTONDOWN: u32 = 519;

const VK_ESCAPE: u32 = 0x1B;
const VK_SPACE: u32 = 0x20;
const VK_RETURN: u32 = 0x0D;

// --- events -------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum InputEvent {
    Key(u32),
    Move(i32, i32),
    Click,
}

static TX: OnceLock<Mutex<mpsc::Sender<InputEvent>>> = OnceLock::new();

fn send(ev: InputEvent) {
    if let Some(tx) = TX.get() {
        if let Ok(tx) = tx.lock() {
            let _ = tx.send(ev);
        }
    }
}

/// Install global hooks; returns the receiver end for the consumer loop.
/// Idempotent sender registration; call once per process.
pub fn install_hooks() -> mpsc::Receiver<InputEvent> {
    let (tx, rx) = mpsc::channel::<InputEvent>();
    let _ = TX.set(Mutex::new(tx));
    std::thread::spawn(hook_thread);
    rx
}

// --- pure gesture evaluation (unit-testable) ------------------------------------

pub fn key_unlocks(mode: UnlockKey, vk: u32) -> bool {
    match mode {
        UnlockKey::Any => true,
        UnlockKey::Esc => vk == VK_ESCAPE,
        UnlockKey::Space => vk == VK_SPACE,
        UnlockKey::Enter => vk == VK_RETURN,
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Stateful shake/move detector over a trailing mouse path.
#[derive(Debug, Default)]
pub struct ShakeTracker {
    anchor: Option<(i32, i32)>,
    trail: VecDeque<(u128, i32, i32)>,
}

impl ShakeTracker {
    pub fn new() -> Self {
        ShakeTracker {
            anchor: None,
            trail: VecDeque::new(),
        }
    }

    pub fn reset(&mut self) {
        self.anchor = None;
        self.trail.clear();
    }

    /// Feed a mouse position. Returns true when `mode` is satisfied.
    pub fn feed(&mut self, mode: UnlockMouse, x: i32, y: i32) -> bool {
        if self.anchor.is_none() {
            self.anchor = Some((x, y));
        }
        let now = now_ms();
        self.trail.push_back((now, x, y));
        while let Some(&(t, _, _)) = self.trail.front() {
            if now.saturating_sub(t) > SHAKE_WINDOW_MS {
                self.trail.pop_front();
            } else {
                break;
            }
        }

        match mode {
            UnlockMouse::Off => false,
            UnlockMouse::Click => false, // handled by Click events
            UnlockMouse::Move => {
                if let Some((ax, ay)) = self.anchor {
                    let dx = (x - ax) as i64;
                    let dy = (y - ay) as i64;
                    dx * dx + dy * dy
                        >= (MOVE_UNLOCK_PX as i64) * (MOVE_UNLOCK_PX as i64)
                } else {
                    false
                }
            }
            UnlockMouse::Shake => {
                let mut flips: u32 = 0;
                let mut last_sign: i32 = 0;
                let mut path: i64 = 0;
                let pts: Vec<(i32, i32)> =
                    self.trail.iter().map(|&(_, px, py)| (px, py)).collect();
                for w in pts.windows(2) {
                    let dx = w[1].0 - w[0].0;
                    let dy = w[1].1 - w[0].1;
                    path += (dx.abs() as i64) + (dy.abs() as i64);
                    if dx.abs() >= SHAKE_MIN_DX {
                        let s = if dx > 0 { 1 } else { -1 };
                        if last_sign != 0 && s != last_sign {
                            flips += 1;
                        }
                        last_sign = s;
                    }
                }
                flips >= SHAKE_FLIPS_NEEDED && path >= SHAKE_MIN_PATH
            }
        }
    }
}

// --- hook procedures (must stay tiny: LowLevelHooksTimeout) ----------------------

unsafe extern "system" fn kb_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION {
        let msg = wparam.0 as u32;
        if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
            let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            send(InputEvent::Key(kb.vkCode));
        }
    }
    CallNextHookEx(HHOOK(std::ptr::null_mut()), code, wparam, lparam)
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION {
        let msg = wparam.0 as u32;
        if msg == WM_MOUSEMOVE {
            let ms = &*(lparam.0 as *const MSLLHOOKSTRUCT);
            send(InputEvent::Move(ms.pt.x, ms.pt.y));
        } else if msg == WM_LBUTTONDOWN || msg == WM_RBUTTONDOWN || msg == WM_MBUTTONDOWN {
            send(InputEvent::Click);
        }
    }
    CallNextHookEx(HHOOK(std::ptr::null_mut()), code, wparam, lparam)
}

fn hook_thread() {
    unsafe {
        // Low-level global hooks run in this thread's context; NULL module is
        // correct for WH_KEYBOARD_LL / WH_MOUSE_LL installed process-locally.
        let hmod = HMODULE(std::ptr::null_mut());
        let kb_fn: unsafe extern "system" fn(i32, WPARAM, LPARAM) -> LRESULT = kb_proc;
        let mouse_fn: unsafe extern "system" fn(i32, WPARAM, LPARAM) -> LRESULT = mouse_proc;
        let kb: HHOOK = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(kb_fn), hmod, 0) {
            Ok(h) => h,
            Err(_) => return,
        };
        let mouse: HHOOK = match SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_fn), hmod, 0) {
            Ok(h) => h,
            Err(_) => {
                let _ = UnhookWindowsHookEx(kb);
                return;
            }
        };
        crate::log::log_line("global input hooks installed");
        // A message pump is mandatory: low-level hooks are dispatched to the
        // installing thread only while it pumps messages.
        let mut msg: MSG = std::mem::zeroed();
        loop {
            let r = GetMessageW(&mut msg, HWND(std::ptr::null_mut()), 0, 0);
            if r.0 == 0 || r.0 == -1 {
                break;
            }
        }
        let _ = UnhookWindowsHookEx(kb);
        let _ = UnhookWindowsHookEx(mouse);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_matrix() {
        assert!(key_unlocks(UnlockKey::Any, 0x41));
        assert!(key_unlocks(UnlockKey::Esc, VK_ESCAPE));
        assert!(!key_unlocks(UnlockKey::Esc, VK_SPACE));
        assert!(key_unlocks(UnlockKey::Space, VK_SPACE));
        assert!(key_unlocks(UnlockKey::Enter, VK_RETURN));
        assert!(!key_unlocks(UnlockKey::Enter, VK_ESCAPE));
    }

    #[test]
    fn straight_line_is_not_a_shake() {
        let mut t = ShakeTracker::new();
        let mut unlocked = false;
        // Long straight horizontal drag: no direction reversals.
        for i in 0..40 {
            unlocked = t.feed(UnlockMouse::Shake, i * 30, 500);
        }
        assert!(!unlocked, "AI-like straight paths must not unlock");
        assert!(!t.feed(UnlockMouse::Off, 9999, 9999));
    }

    #[test]
    fn human_shake_unlocks() {
        let mut t = ShakeTracker::new();
        let mut unlocked = false;
        // Deliberate side-to-side shake with ample strokes.
        let mut x = 500;
        let mut dir = 1;
        for _ in 0..10 {
            for _ in 0..4 {
                x += dir * 60;
                unlocked = t.feed(UnlockMouse::Shake, x, 500);
                if unlocked {
                    break;
                }
            }
            if unlocked {
                break;
            }
            dir = -dir;
        }
        assert!(unlocked, "a real shake must unlock");
    }

    #[test]
    fn move_threshold() {
        let mut t = ShakeTracker::new();
        assert!(!t.feed(UnlockMouse::Move, 100, 100)); // anchor
        assert!(!t.feed(UnlockMouse::Move, 110, 105)); // small jitter
        assert!(t.feed(UnlockMouse::Move, 100 + MOVE_UNLOCK_PX + 5, 100));
    }
}
