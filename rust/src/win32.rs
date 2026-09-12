//! Raw Win32 FFI with explicit, stable signatures.
//!
//! Everything here mirrors the Win32 API 1:1 (HWND as `isize`, stable
//! constant values), so this module cannot break on dependency updates.

use std::ffi::c_void;

// --- capture exclusion / click-through --------------------------------------

/// WDA_EXCLUDEFROMCAPTURE: visible on the physical monitor only; excluded
/// from screenshots, recordings, DWM/Desktop-Duplication capture and AI vision.
pub const WDA_EXCLUDEFROMCAPTURE: u32 = 0x00000011;

pub const GWL_EXSTYLE: i32 = -20;
pub const WS_EX_TRANSPARENT: isize = 0x00000020;
pub const WS_EX_LAYERED: isize = 0x00080000;
pub const WS_EX_TOOLWINDOW: isize = 0x00000080;
pub const WS_EX_TOPMOST: isize = 0x00000008;
pub const WS_EX_NOACTIVATE: isize = 0x08000000;

// --- window creation / messages ----------------------------------------------

pub const WS_POPUP: u32 = 0x80000000;
pub const WS_VISIBLE: u32 = 0x10000000;
pub const SW_SHOWNOACTIVATE: i32 = 4;
pub const SWP_NOACTIVATE: u32 = 0x0010;
pub const SWP_SHOWWINDOW: u32 = 0x0040;
pub const SWP_NOMOVE: u32 = 0x0002;
pub const SWP_NOSIZE: u32 = 0x0001;
pub const HWND_TOPMOST: isize = -1;
pub const BLACK_BRUSH: i32 = 4;

pub const WM_PAINT: u32 = 0x000F;
pub const WM_DISPLAYCHANGE: u32 = 0x007E;
pub const WM_CLOSE: u32 = 0x0010;
pub const WM_QUIT: u32 = 0x0012;
pub const SW_RESTORE: i32 = 9;
pub const ERROR_ALREADY_EXISTS: u32 = 183;

/// Per-monitor-V2 DPI awareness (Win10 1607+; target is Win11+).
/// Threads that create/position windows must opt in, otherwise the OS
/// virtualizes coordinates and fullscreen overlays land off-target.
pub const DPI_AWARENESS_PER_MONITOR_V2: isize = -4;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[repr(C)]
pub struct Paint {
    pub hdc: isize,
    pub erase: i32,
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub restore: i32,
    pub update: i32,
    pub rgb: [u8; 32],
}

#[repr(C)]
pub struct WinMsg {
    pub hwnd: isize,
    pub message: u32,
    pub _pad: u32,
    pub wparam: usize,
    pub lparam: isize,
    pub time: u32,
    pub pt_x: i32,
    pub pt_y: i32,
}

#[repr(C)]
pub struct MonitorInfo {
    pub cb_size: u32,
    pub rc_monitor: Rect,
    pub rc_work: Rect,
    pub flags: u32,
}

pub type WndProc = Option<unsafe extern "system" fn(isize, u32, usize, isize) -> isize>;
pub type EnumMonProc =
    Option<unsafe extern "system" fn(isize, isize, *mut Rect, isize) -> i32>;

#[repr(C)]
pub struct WndClass {
    pub cb_size: u32,
    pub style: u32,
    pub wnd_proc: WndProc,
    pub cls_extra: i32,
    pub wnd_extra: i32,
    pub h_instance: isize,
    pub h_icon: isize,
    pub h_cursor: isize,
    pub h_background: isize,
    pub menu_name: *const u16,
    pub class_name: *const u16,
    pub h_icon_sm: isize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[link(name = "user32")]
extern "system" {
    fn SetWindowDisplayAffinity(hwnd: isize, affinity: u32) -> i32;
    fn GetWindowLongPtrW(hwnd: isize, index: i32) -> isize;
    fn SetWindowLongPtrW(hwnd: isize, index: i32, newlong: isize) -> isize;
    fn SetLayeredWindowAttributes(hwnd: isize, key: u32, alpha: u8, flags: u32) -> i32;
    fn GetLayeredWindowAttributes(
        hwnd: isize,
        key: *mut u32,
        alpha: *mut u8,
        flags: *mut u32,
    ) -> i32;
    fn FindWindowW(classname: *const u16, windowname: *const u16) -> isize;
    fn RegisterClassExW(cls: *const WndClass) -> u16;
    fn CreateWindowExW(
        exstyle: u32,
        classname: *const u16,
        title: *const u16,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: isize,
        menu: isize,
        instance: isize,
        param: *const c_void,
    ) -> isize;
    fn DefWindowProcW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
    fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
    fn IsWindowVisible(hwnd: isize) -> i32;
    fn SetWindowPos(
        hwnd: isize,
        after: isize,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        flags: u32,
    ) -> i32;
    fn DestroyWindow(hwnd: isize) -> i32;
    fn BeginPaint(hwnd: isize, paint: *mut Paint) -> isize;
    fn EndPaint(hwnd: isize, paint: *const Paint) -> i32;
    fn FillRect(hdc: isize, rect: *const Rect, brush: isize) -> i32;
    fn GetStockObject(i: i32) -> isize;
    fn PostThreadMessageW(thread: u32, msg: u32, wparam: usize, lparam: isize) -> i32;
    fn GetMessageW(msg: *mut WinMsg, hwnd: isize, min: u32, max: u32) -> i32;
    fn TranslateMessage(msg: *const WinMsg) -> i32;
    fn DispatchMessageW(msg: *const WinMsg) -> isize;
    fn PostQuitMessage(code: i32);
    fn EnumDisplayMonitors(
        hdc: isize,
        clip: *const Rect,
        proc_: EnumMonProc,
        data: isize,
    ) -> i32;
    fn GetMonitorInfoW(hmon: isize, info: *mut MonitorInfo) -> i32;
    fn GetWindowRect(hwnd: isize, rect: *mut Rect) -> i32;
    fn SetThreadDpiAwarenessContext(ctx: isize) -> isize;
    fn SetForegroundWindow(hwnd: isize) -> i32;
    fn CreateMutexW(attr: *const c_void, owner: i32, name: *const u16) -> isize;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetTickCount() -> u32;
    fn GetModuleHandleW(name: *const u16) -> isize;
    fn GetCurrentThreadId() -> u32;
    fn GetLastError() -> u32;
}

pub fn tick_ms() -> u32 {
    unsafe { GetTickCount() }
}

pub fn module_handle() -> isize {
    unsafe { GetModuleHandleW(std::ptr::null()) }
}

pub fn current_thread_id() -> u32 {
    unsafe { GetCurrentThreadId() }
}

/// Opt the calling thread into per-monitor-V2 DPI awareness so window
/// coordinates are physical pixels (no OS virtualization).
pub fn enable_per_monitor_dpi() {
    unsafe {
        SetThreadDpiAwarenessContext(DPI_AWARENESS_PER_MONITOR_V2);
    }
}

fn to_wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Find a top-level window by exact title. Returns raw HWND (0 = missing).
pub fn find_window_by_title(title: &str) -> isize {
    let wide = to_wide_null(title);
    unsafe { FindWindowW(std::ptr::null(), wide.as_ptr()) }
}

/// Exclude `hwnd` from all capture pipelines. Returns true on success.
pub fn apply_capture_exclusion(hwnd: isize) -> bool {
    if hwnd == 0 {
        return false;
    }
    unsafe { SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE) != 0 }
}

/// Make `hwnd` click-through (WS_EX_TRANSPARENT | WS_EX_LAYERED).
pub fn apply_click_through(hwnd: isize) {
    if hwnd == 0 {
        return;
    }
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | WS_EX_TRANSPARENT | WS_EX_LAYERED);
    }
}

pub const LWA_ALPHA: u32 = 0x2;

/// A window *created* with WS_EX_LAYERED stays invisible until
/// SetLayeredWindowAttributes/UpdateLayeredWindow is called. Lock the layer
/// at alpha=255 so the black overlay is actually composed on screen.
pub fn apply_opaque_layer(hwnd: isize) -> bool {
    if hwnd == 0 {
        return false;
    }
    unsafe { SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA) != 0 }
}

/// Current (alpha, flags) of a layered window, for tests. None if the call
/// fails (e.g. the window is not layered).
pub fn layered_alpha(hwnd: isize) -> Option<(u8, u32)> {
    if hwnd == 0 {
        return None;
    }
    unsafe {
        let mut key: u32 = 0;
        let mut alpha: u8 = 0;
        let mut flags: u32 = 0;
        if GetLayeredWindowAttributes(hwnd, &mut key, &mut alpha, &mut flags) != 0 {
            Some((alpha, flags))
        } else {
            None
        }
    }
}

pub fn wide_null(s: &str) -> Vec<u16> {
    to_wide_null(s)
}

pub fn register_class(cls: &WndClass) -> bool {
    unsafe { RegisterClassExW(cls) != 0 }
}

pub fn create_window(
    exstyle: u32,
    class: &[u16],
    title: &[u16],
    style: u32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    instance: isize,
) -> isize {
    unsafe {
        CreateWindowExW(
            exstyle,
            class.as_ptr(),
            title.as_ptr(),
            style,
            x,
            y,
            w,
            h,
            0,
            0,
            instance,
            std::ptr::null(),
        )
    }
}

pub fn def_window_proc(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

pub fn show_noactivate(hwnd: isize) {
    unsafe {
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        // NOTE: SWP_NOMOVE | SWP_NOSIZE is mandatory here — without it the
        // window would be moved to (0,0) and resized to 0x0.
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_NOMOVE | SWP_NOSIZE,
        );
    }
}

/// Whether the OS considers `hwnd` shown (IsWindowVisible).
pub fn is_window_visible(hwnd: isize) -> bool {
    unsafe { IsWindowVisible(hwnd) != 0 }
}

pub fn destroy_window(hwnd: isize) {
    unsafe {
        DestroyWindow(hwnd);
    }
}

pub fn post_quit() {
    unsafe {
        PostQuitMessage(0);
    }
}

pub fn post_thread_close(thread_id: u32) {
    unsafe {
        PostThreadMessageW(thread_id, WM_CLOSE, 0, 0);
    }
}
pub fn pump_messages() {
    unsafe {
        let mut msg: WinMsg = std::mem::zeroed();
        loop {
            let r = GetMessageW(&mut msg, 0, 0, 0);
            if r == 0 || r == -1 {
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Single message-pump step helpers (for threads that must intercept
/// thread-messages like WM_CLOSE before dispatch).
pub fn get_message(msg: *mut WinMsg) -> i32 {
    unsafe { GetMessageW(msg, 0, 0, 0) }
}

pub fn translate_message(msg: *const WinMsg) {
    unsafe {
        TranslateMessage(msg);
    }
}

pub fn dispatch_message(msg: *const WinMsg) {
    unsafe {
        DispatchMessageW(msg);
    }
}

/// Move/resize + keep topmost without activating.
pub fn place_window(hwnd: isize, x: i32, y: i32, w: i32, h: i32) {
    unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            x,
            y,
            w,
            h,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}

pub fn paint_black(hwnd: isize) {
    unsafe {
        let mut ps: Paint = std::mem::zeroed();
        let hdc = BeginPaint(hwnd, &mut ps);
        if hdc != 0 {
            let rc = Rect {
                left: ps.left,
                top: ps.top,
                right: ps.right,
                bottom: ps.bottom,
            };
            let brush = GetStockObject(BLACK_BRUSH);
            FillRect(hdc, &rc, brush);
        }
        EndPaint(hwnd, &ps);
    }
}

struct MonitorCollector {
    out: Vec<MonitorRect>,
}

unsafe extern "system" fn enum_mon_proc(
    hmon: isize,
    _hdc: isize,
    _rect: *mut Rect,
    data: isize,
) -> i32 {
    let collector = &mut *(data as *mut MonitorCollector);
    let mut info: MonitorInfo = std::mem::zeroed();
    info.cb_size = std::mem::size_of::<MonitorInfo>() as u32;
    if GetMonitorInfoW(hmon, &mut info) != 0 {
        let r = info.rc_monitor;
        collector.out.push(MonitorRect {
            x: r.left,
            y: r.top,
            w: r.right - r.left,
            h: r.bottom - r.top,
        });
    }
    1 // TRUE = continue
}

/// Physical pixel geometry of all monitors.
pub fn enum_monitors() -> Vec<MonitorRect> {
    let mut collector = MonitorCollector { out: Vec::new() };
    unsafe {
        EnumDisplayMonitors(
            0,
            std::ptr::null(),
            Some(enum_mon_proc),
            &mut collector as *mut MonitorCollector as isize,
        );
    }
    collector.out
}

/// Current window rect (client+frame) or None.
pub fn window_rect(hwnd: isize) -> Option<Rect> {    let mut rc: Rect = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let ok = unsafe { GetWindowRect(hwnd, &mut rc) != 0 };
    if ok {
        Some(rc)
    } else {
        None
    }
}

/// Take a named process-singleton mutex. Returns true when this process is
/// the first owner (false = another instance is already running).
pub fn take_single_instance(name: &str) -> bool {
    let wide = to_wide_null(name);
    unsafe {
        CreateMutexW(std::ptr::null(), 0, wide.as_ptr());
        GetLastError() != ERROR_ALREADY_EXISTS
    }
}

/// Bring an existing top-level window to the foreground (used to focus the
/// first instance's settings window). Returns true if found.
pub fn focus_window_by_title(title: &str) -> bool {
    let hwnd = find_window_by_title(title);
    if hwnd == 0 {
        return false;
    }
    unsafe {
        ShowWindow(hwnd, SW_RESTORE);
        SetForegroundWindow(hwnd) != 0
    }
}

#[cfg(test)]
mod ffi_layout_tests {
    use super::*;
    use std::mem::{align_of, size_of};

    #[test]
    fn struct_layout_matches_win32_x64() {
        assert_eq!(size_of::<Rect>(), 16);
        // PAINTSTRUCT fields end at 68 but 8-byte alignment pads it to 72.
        // Field offsets (what BeginPaint/EndPaint actually use) are exact.
        assert_eq!(size_of::<Paint>(), 72);
        assert_eq!(size_of::<WinMsg>(), 48);
        assert_eq!(size_of::<MonitorInfo>(), 40);
        assert_eq!(size_of::<WndClass>(), 80);
        assert_eq!(align_of::<WinMsg>(), 8);
    }

    #[test]
    fn monitors_detected() {
        let mons = enum_monitors();
        assert!(!mons.is_empty(), "at least the primary monitor must exist");
        for m in &mons {
            assert!(m.w > 0 && m.h > 0);
        }
    }
}
