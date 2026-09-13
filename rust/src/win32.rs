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
pub const SW_HIDE: i32 = 0;
pub const ERROR_ALREADY_EXISTS: u32 = 183;
/// Base for private thread messages (OSD repaint requests use WM_APP + n).
pub const WM_APP: u32 = 0x8000;

// --- GDI shapes / layered composition (OSD) -----------------------------------

pub const PS_SOLID: i32 = 0;
pub const NULL_BRUSH: i32 = 5;
pub const NULL_PEN: i32 = 8;
pub const BI_RGB: u32 = 0;
pub const DIB_RGB_COLORS: u32 = 0;
pub const ULW_ALPHA: u32 = 2;
pub const AC_SRC_OVER: u8 = 0;
pub const AC_SRC_ALPHA: u8 = 1;
pub const MONITORINFOF_PRIMARY: u32 = 1;

// --- GDI text (blackout unlock hint) -----------------------------------------

pub const BKMODE_TRANSPARENT: i32 = 1;
pub const TA_CENTER: u32 = 6;
pub const FW_NORMAL: i32 = 400;
pub const DEFAULT_CHARSET: u32 = 1;
pub const OUT_DEFAULT_PRECIS: u32 = 0;
pub const CLIP_DEFAULT_PRECIS: u32 = 0;
pub const CLEARTYPE_QUALITY: u32 = 5;
pub const DEFAULT_PITCH: u32 = 0;

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

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub cx: i32,
    pub cy: i32,
}

#[repr(C)]
pub struct BitmapInfoHeader {
    pub size: u32,
    pub width: i32,
    pub height: i32,
    pub planes: u16,
    pub bit_count: u16,
    pub compression: u32,
    pub size_image: u32,
    pub xppm: i32,
    pub yppm: i32,
    pub clr_used: u32,
    pub clr_important: u32,
}

#[repr(C)]
pub struct BitmapInfo {
    pub header: BitmapInfoHeader,
    pub colors: [u8; 4],
}

#[repr(C)]
pub struct BlendFunction {
    pub blend_op: u8,
    pub blend_flags: u8,
    pub source_constant_alpha: u8,
    pub alpha_format: u8,
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
    /// True for the primary monitor.
    pub primary: bool,
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
    fn GetClientRect(hwnd: isize, rect: *mut Rect) -> i32;
    fn SetThreadDpiAwarenessContext(ctx: isize) -> isize;
    fn SetForegroundWindow(hwnd: isize) -> i32;
    fn CreateMutexW(attr: *const c_void, owner: i32, name: *const u16) -> isize;
    fn UpdateLayeredWindow(
        hwnd: isize,
        dst_dc: isize,
        dst_pt: *const Point,
        size: *const Size,
        src_dc: isize,
        src_pt: *const Point,
        key: u32,
        blend: *const BlendFunction,
        flags: u32,
    ) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateFontW(
        height: i32,
        width: i32,
        escapement: i32,
        orientation: i32,
        weight: i32,
        italic: u32,
        underline: u32,
        strikeout: u32,
        charset: u32,
        out_precision: u32,
        clip_precision: u32,
        quality: u32,
        pitch_and_family: u32,
        face: *const u16,
    ) -> isize;
    fn SelectObject(hdc: isize, obj: isize) -> isize;
    fn DeleteObject(obj: isize) -> i32;
    fn SetTextColor(hdc: isize, color: u32) -> u32;
    fn SetBkMode(hdc: isize, mode: i32) -> i32;
    fn SetTextAlign(hdc: isize, align: u32) -> u32;
    fn TextOutW(hdc: isize, x: i32, y: i32, s: *const u16, count: i32) -> i32;
    fn CreateCompatibleDC(hdc: isize) -> isize;
    fn DeleteDC(hdc: isize) -> i32;
    fn CreateDIBSection(
        hdc: isize,
        bmi: *const BitmapInfo,
        usage: u32,
        bits: *mut *mut c_void,
        section: isize,
        offset: u32,
    ) -> isize;
    fn CreateSolidBrush(color: u32) -> isize;
    fn CreatePen(style: i32, width: i32, color: u32) -> isize;
    fn MoveToEx(hdc: isize, x: i32, y: i32, previous: *mut Point) -> i32;
    fn LineTo(hdc: isize, x: i32, y: i32) -> i32;
    fn Ellipse(hdc: isize, l: i32, t: i32, r: i32, b: i32) -> i32;
    fn RoundRect(
        hdc: isize,
        l: i32,
        t: i32,
        r: i32,
        b: i32,
        edge_w: i32,
        edge_h: i32,
    ) -> i32;
    fn GdiFlush() -> i32;
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

/// Post a private thread message (e.g. WM_APP + n) to a pump thread.
pub fn post_thread_msg(thread_id: u32, msg: u32) {
    unsafe {
        PostThreadMessageW(thread_id, msg, 0, 0);
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

/// Begin a WM_PAINT cycle. Returns the paint DC (0 on failure).
pub fn begin_paint(hwnd: isize, ps: &mut Paint) -> isize {
    unsafe { BeginPaint(hwnd, ps) }
}

/// End a WM_PAINT cycle begun with [`begin_paint`] (mandatory, even on error).
pub fn end_paint(hwnd: isize, ps: &Paint) -> i32 {
    unsafe { EndPaint(hwnd, ps as *const Paint) }
}

/// Fill `rc` with the stock black brush.
pub fn fill_black(hdc: isize, rc: &Rect) {
    unsafe {
        FillRect(hdc, rc as *const Rect, GetStockObject(BLACK_BRUSH));
    }
}

/// Client-area rect (== window rect for WS_POPUP overlays). None on failure.
pub fn client_rect(hwnd: isize) -> Option<Rect> {
    let mut rc: Rect = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let ok = unsafe { GetClientRect(hwnd, &mut rc) != 0 };
    if ok {
        Some(rc)
    } else {
        None
    }
}

/// Draw one line of `text` horizontally centered at `cx`, top edge at `y`,
/// in Malgun Gothic at `font_px` with `color` (COLORREF 0x00BBGGRR) on a
/// transparent background. Best-effort: silently skips on font failure.
pub fn draw_centered_text(hdc: isize, cx: i32, y: i32, text: &str, font_px: i32, color: u32) {
    unsafe {
        let face = wide_null("Malgun Gothic");
        let font = CreateFontW(
            font_px,
            0,
            0,
            0,
            FW_NORMAL,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            DEFAULT_PITCH,
            face.as_ptr(),
        );
        if font == 0 {
            return;
        }
        let old = SelectObject(hdc, font);
        SetTextColor(hdc, color);
        SetBkMode(hdc, BKMODE_TRANSPARENT);
        SetTextAlign(hdc, TA_CENTER);
        let wide = wide_null(text);
        TextOutW(hdc, cx, y, wide.as_ptr(), (wide.len() - 1) as i32);
        SelectObject(hdc, old);
        DeleteObject(font);
    }
}

// --- OSD drawing / layered composition -----------------------------------------

/// Stock object handle by id (NULL_BRUSH, NULL_PEN, ...).
pub fn stock_obj(which: i32) -> isize {
    unsafe { GetStockObject(which) }
}

/// Select a GDI object into a DC; returns the previously selected object.
pub fn select_object(hdc: isize, obj: isize) -> isize {
    unsafe { SelectObject(hdc, obj) }
}

/// Delete a GDI object (must not be currently selected into a live DC).
pub fn delete_object(obj: isize) {
    unsafe {
        DeleteObject(obj);
    }
}

/// Solid brush in `color` (COLORREF). Caller must DeleteObject.
pub fn create_solid_brush(color: u32) -> isize {
    unsafe { CreateSolidBrush(color) }
}

/// Solid pen (width in px) in `color` (COLORREF). Caller must DeleteObject.
pub fn create_pen(width: i32, color: u32) -> isize {
    unsafe { CreatePen(PS_SOLID, width, color) }
}

pub fn move_to(hdc: isize, x: i32, y: i32) {
    unsafe {
        MoveToEx(hdc, x, y, std::ptr::null_mut());
    }
}

pub fn line_to(hdc: isize, x: i32, y: i32) {
    unsafe {
        LineTo(hdc, x, y);
    }
}

/// Outline/filled ellipse with the currently selected pen and brush.
pub fn ellipse_shape(hdc: isize, l: i32, t: i32, r: i32, b: i32) {
    unsafe {
        Ellipse(hdc, l, t, r, b);
    }
}

/// Rounded rectangle with the currently selected pen and brush.
pub fn round_rect_shape(hdc: isize, l: i32, t: i32, r: i32, b: i32, edge: i32) {
    unsafe {
        RoundRect(hdc, l, t, r, b, edge, edge);
    }
}

/// Fill `rc` with an opaque `color`.
pub fn fill_color(hdc: isize, rc: &Rect, color: u32) {
    unsafe {
        let brush = CreateSolidBrush(color);
        if brush != 0 {
            FillRect(hdc, rc as *const Rect, brush);
            DeleteObject(brush);
        }
    }
}

pub fn hide_window(hwnd: isize) {
    unsafe {
        ShowWindow(hwnd, SW_HIDE);
    }
}

/// Re-insert `hwnd` at the top of the topmost band without activating.
pub fn reassert_topmost(hwnd: isize) {
    unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
        );
    }
}

/// A top-down 32bpp ARGB DIB bound to a memory DC. GDI draws into `dc`;
/// the resulting pixels are readable/writable through `bits`.
pub struct ArgbSurface {
    pub dc: isize,
    pub bitmap: isize,
    pub bits: *mut u32,
    old_bitmap: isize,
    pub w: i32,
    pub h: i32,
}

impl Drop for ArgbSurface {
    fn drop(&mut self) {
        unsafe {
            if self.dc != 0 {
                if self.old_bitmap != 0 {
                    SelectObject(self.dc, self.old_bitmap);
                }
                DeleteDC(self.dc);
            }
            if self.bitmap != 0 {
                DeleteObject(self.bitmap);
            }
        }
    }
}

/// Create a `w` x `h` ARGB surface initialized to transparent black.
pub fn create_argb_surface(w: i32, h: i32) -> Option<ArgbSurface> {
    if w <= 0 || h <= 0 {
        return None;
    }
    unsafe {
        let dc = CreateCompatibleDC(0);
        if dc == 0 {
            return None;
        }
        let bmi = BitmapInfo {
            header: BitmapInfoHeader {
                size: std::mem::size_of::<BitmapInfoHeader>() as u32,
                width: w,
                height: -h, // top-down rows
                planes: 1,
                bit_count: 32,
                compression: BI_RGB,
                size_image: 0,
                xppm: 0,
                yppm: 0,
                clr_used: 0,
                clr_important: 0,
            },
            colors: [0; 4],
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let bitmap = CreateDIBSection(
            dc,
            &bmi,
            DIB_RGB_COLORS,
            &mut bits,
            0,
            0,
        );
        if bitmap == 0 || bits.is_null() {
            DeleteDC(dc);
            return None;
        }
        let old_bitmap = SelectObject(dc, bitmap);
        Some(ArgbSurface {
            dc,
            bitmap,
            bits: bits as *mut u32,
            old_bitmap,
            w,
            h,
        })
    }
}

/// Push `surface` to the layered `hwnd` as premultiplied ARGB, moving the
/// window to (`x`, `y`) and sizing it to the surface. GDI writes carry no
/// alpha, so `finalize_argb` must have run first.
pub fn update_layered_pixels(hwnd: isize, x: i32, y: i32, surface: &ArgbSurface) -> bool {
    unsafe {
        GdiFlush();
        let dst_pt = Point { x, y };
        let size = Size {
            cx: surface.w,
            cy: surface.h,
        };
        let src_pt = Point { x: 0, y: 0 };
        let blend = BlendFunction {
            blend_op: AC_SRC_OVER,
            blend_flags: 0,
            source_constant_alpha: 255,
            alpha_format: AC_SRC_ALPHA,
        };
        UpdateLayeredWindow(
            hwnd,
            0,
            &dst_pt,
            &size,
            surface.dc,
            &src_pt,
            0,
            &blend,
            ULW_ALPHA,
        ) != 0
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
            primary: info.flags & MONITORINFOF_PRIMARY != 0,
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

/// The primary monitor (taskbar/default monitor), or the first enumerated
/// one as a fallback.
pub fn primary_monitor() -> MonitorRect {
    let mons = enum_monitors();
    mons
        .iter()
        .find(|m| m.primary)
        .copied()
        .unwrap_or_else(|| MonitorRect {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
            primary: true,
        })
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
