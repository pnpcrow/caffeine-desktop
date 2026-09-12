//! Capture-proof blackout overlay: one pure-Win32 window per monitor.
//!
//! No GUI framework is involved: the windows are created with
//! `CreateWindowExW`, hardened with `SetWindowDisplayAffinity
//! (WDA_EXCLUDEFROMCAPTURE)` and made click-through with
//! `WS_EX_TRANSPARENT | WS_EX_LAYERED`, then shown without activation.
//! A dedicated thread pumps their messages; closing is initiated from any
//! thread via `PostThreadMessageW(WM_CLOSE)`.
//!
//! Lifetime contract: the host must keep the `OverlayManager` alive until
//! process exit (e.g. in a `static`). The pump thread borrows it as
//! `&'static` under that contract.

use crate::win32::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

const CLASS_NAME_STR: &str = "CaffeineGuardOverlay";

static CLASS_NAME: OnceLock<&'static [u16]> = OnceLock::new();
static CLASS_ONCE: OnceLock<()> = OnceLock::new();
/// Live overlay HWNDs (for display-change re-layout).
static HWNDS: OnceLock<Mutex<Vec<isize>>> = OnceLock::new();

fn hwnds() -> &'static Mutex<Vec<isize>> {
    HWNDS.get_or_init(|| Mutex::new(Vec::new()))
}

fn class_name() -> &'static [u16] {
    CLASS_NAME.get_or_init(|| Box::leak(wide_null(CLASS_NAME_STR).into_boxed_slice()))
}

unsafe extern "system" fn wnd_proc(
    hwnd: isize,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    match msg {
        WM_PAINT => {
            paint_black(hwnd);
            0
        }
        WM_DISPLAYCHANGE => {
            // Monitors changed while covered: re-fit every overlay.
            for (i, mon) in enum_monitors().iter().enumerate() {
                if let Ok(list) = hwnds().lock() {
                    if let Some(&h) = list.get(i) {
                        place_window(h, mon.x, mon.y, mon.w, mon.h);
                    }
                }
            }
            def_window_proc(hwnd, msg, wparam, lparam)
        }
        _ => def_window_proc(hwnd, msg, wparam, lparam),
    }
}

fn ensure_class(instance: isize) {
    CLASS_ONCE.get_or_init(|| {
        let cls = WndClass {
            cb_size: std::mem::size_of::<WndClass>() as u32,
            style: 0,
            wnd_proc: Some(wnd_proc),
            cls_extra: 0,
            wnd_extra: 0,
            h_instance: instance,
            h_icon: 0,
            h_cursor: 0,
            h_background: 0,
            menu_name: std::ptr::null(),
            class_name: class_name().as_ptr(),
            h_icon_sm: 0,
        };
        register_class(&cls);
    });
}

pub struct OverlayManager {
    alive: AtomicBool,
    thread_id: Mutex<Option<u32>>,
}

impl OverlayManager {
    pub fn new() -> Self {
        OverlayManager {
            alive: AtomicBool::new(false),
            thread_id: Mutex::new(None),
        }
    }

    pub fn is_showing(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// Show the blackout on all monitors (no-op if already showing).
    ///
    /// SAFETY contract: the manager must outlive the pump thread (host keeps
    /// it for the process lifetime); the thread only reads `alive` and
    /// `thread_id`.
    pub fn show(&self) {
        if self.alive.swap(true, Ordering::SeqCst) {
            return;
        }
        // Extend the borrow to 'static under the lifetime contract above.
        let this: &'static Self = unsafe { &*(self as *const Self) };
        std::thread::spawn(move || {
            overlay_thread(this);
        });
    }

    /// Hide and destroy all overlay windows (no-op if not showing).
    pub fn hide(&self) {
        if !self.alive.swap(false, Ordering::SeqCst) {
            return;
        }
        crate::log::log_line("blackout cleared");
        if let Ok(guard) = self.thread_id.lock() {
            if let Some(tid) = *guard {
                post_thread_close(tid);
            }
        }
    }
}

impl Default for OverlayManager {
    fn default() -> Self {
        Self::new()
    }
}

fn overlay_thread(mgr: &'static OverlayManager) {
    enable_per_monitor_dpi();
    let instance = module_handle();
    ensure_class(instance);
    if let Ok(mut guard) = mgr.thread_id.lock() {
        *guard = Some(current_thread_id());
    }

    // hide() may have completed between show()'s spawn and this point; its
    // PostThreadMessageW could not find a registered thread id yet, so the
    // only reliable signal is the flag itself. Never create windows then.
    if !mgr.alive.load(Ordering::SeqCst) {
        crate::log::log_line("overlay thread: hidden before start");
        return;
    }

    let monitors = enum_monitors();
    crate::log::log_line(&format!("blackout: {} monitor(s)", monitors.len()));
    let exstyle =
        (WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE)
            as u32;
    let style = WS_POPUP | WS_VISIBLE;
    let mut created: Vec<isize> = Vec::new();
    for (i, mon) in monitors.iter().enumerate() {
        let title = wide_null(&format!("CaffeineGuard-{i}"));
        let hwnd = create_window(
            exstyle,
            class_name(),
            &title,
            style,
            mon.x,
            mon.y,
            mon.w,
            mon.h,
            instance,
        );
        if hwnd != 0 {
            let excluded = apply_capture_exclusion(hwnd);
            apply_click_through(hwnd);
            // Mandatory: a window born WS_EX_LAYERED is not composed on screen
            // until its layer attributes are set — without this the overlay
            // exists but covers nothing (capture exclusion still works, so the
            // bug is invisible to screenshots and only humans notice).
            let opaque = apply_opaque_layer(hwnd);
            show_noactivate(hwnd);
            crate::log::log_line(&format!(
                "overlay-{i}: hwnd={hwnd} capture_excluded={excluded} opaque_layer={opaque}"
            ));
            created.push(hwnd);
        } else {
            crate::log::log_line(&format!("overlay-{i}: creation failed"));
        }
    }
    if let Ok(mut list) = hwnds().lock() {
        list.clear();
        list.extend(created.iter().copied());
    }

    // Same race as above, but for a hide() that landed *during* window
    // creation: its WM_CLOSE sits in this thread's queue and would be pumped
    // only after the screens had gone black for a visible interval. Destroy
    // immediately instead (the queued close then becomes a no-op).
    if !mgr.alive.load(Ordering::SeqCst) {
        crate::log::log_line("overlay thread: hidden during creation");
        destroy_created(&created);
        return;
    }

    // Pump until WM_CLOSE (posted by hide()) or WM_QUIT.
    unsafe {
        let mut msg: WinMsg = std::mem::zeroed();
        loop {
            let r = get_message(&mut msg);
            if r == 0 || r == -1 {
                crate::log::log_line(&format!(
                    "overlay thread: pump exit r={r} msg={:#x}",
                    msg.message
                ));
                break;
            }
            if msg.message == WM_CLOSE && msg.hwnd == 0 {
                crate::log::log_line("overlay thread: WM_CLOSE received");
                break;
            }
            translate_message(&msg);
            dispatch_message(&msg);
        }
    }

    // Destroy only what this generation created: a fast hide()/show() cycle
    // may already have spawned a successor whose handles now own the shared
    // list, and killing those would cancel the new blackout.
    destroy_created(&created);
}

/// Destroy a generation's overlay windows and drop them from the shared list
/// (leaving any successor's handles untouched).
fn destroy_created(created: &[isize]) {
    for &h in created {
        destroy_window(h);
    }
    if let Ok(mut list) = hwnds().lock() {
        list.retain(|h| !created.contains(h));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn show_and_hide_real_windows() {
        // Match the overlay thread's DPI context: without this, Win32
        // virtualizes this thread's GetWindowRect into logical pixels while
        // monitor enumeration stays physical, and the geometry asserts flap
        // depending on the host process's DPI awareness.
        enable_per_monitor_dpi();
        // Briefly covers the screens (~1s). Runs on the real desktop.
        let mgr = OverlayManager::new();
        // Leak to satisfy the 'static contract for the duration of the test
        // (process exit reclaims it; tests are short-lived).
        let mgr: &'static OverlayManager = Box::leak(Box::new(mgr));
        mgr.show();

        let mons = enum_monitors();
        assert!(!mons.is_empty());

        // Wait for all overlay windows to appear.
        let mut ok = false;
        for _ in 0..30 {
            std::thread::sleep(Duration::from_millis(100));
            if !mgr.is_showing() {
                continue;
            }
            ok = (0..mons.len()).all(|i| {
                find_window_by_title(&format!("CaffeineGuard-{i}")) != 0
            });
            if ok {
                break;
            }
        }
        assert!(ok, "overlay windows did not appear");

        // Geometry must match the monitors exactly.
        for (i, mon) in mons.iter().enumerate() {
            let hwnd = find_window_by_title(&format!("CaffeineGuard-{i}"));
            let rc = window_rect(hwnd).expect("overlay rect readable");
            assert_eq!(rc.left, mon.x, "monitor {i} x");
            assert_eq!(rc.top, mon.y, "monitor {i} y");
            assert_eq!(rc.right - rc.left, mon.w, "monitor {i} w");
            assert_eq!(rc.bottom - rc.top, mon.h, "monitor {i} h");
        }

        // The layer must actually compose: a WS_EX_LAYERED window without
        // layer attributes is never shown, so the screens would not be black.
        // This is exactly the regression where "blackout" covered nothing.
        for i in 0..mons.len() {
            let hwnd = find_window_by_title(&format!("CaffeineGuard-{i}"));
            assert!(is_window_visible(hwnd), "overlay {i} visible");
            let (alpha, flags) = layered_alpha(hwnd).expect("overlay {i} is layered");
            assert_eq!(alpha, 255, "overlay {i} alpha must be opaque");
            assert_ne!(flags & LWA_ALPHA, 0, "overlay {i} LWA_ALPHA set");
        }

        mgr.hide();
        std::thread::sleep(Duration::from_millis(800));
        assert!(!mgr.is_showing());
        for i in 0..mons.len() {
            assert_eq!(
                find_window_by_title(&format!("CaffeineGuard-{i}")),
                0,
                "overlay {i} destroyed"
            );
        }
    }
}
