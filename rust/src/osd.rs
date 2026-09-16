//! Tiny status OSD: an always-available, click-through, capture-excluded
//! indicator showing which protections are on.
//!
//! One icon per active feature — a coffee cup for keep-awake, a crossed-out
//! eye for auto-blackout — anchored at one of 9 user-selectable positions on
//! the primary monitor. Like the blackout overlay it is a raw Win32 window:
//! `WDA_EXCLUDEFROMCAPTURE` (human eyes only), `WS_EX_TRANSPARENT` (never
//! blocks input) and `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW` (no taskbar, no
//! focus, no Alt-Tab). Pixels are composed per-update via
//! `UpdateLayeredWindow` from a 32bpp ARGB DIB drawn with hard-edged GDI
//! shapes, so no GUI framework is involved.
//!
//! Hidden while the blackout itself is showing: the black screens already
//! communicate "protected", and this keeps the unlock hint unobstructed.

use crate::settings::OsdPosition;
use crate::win32::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

const CLASS_NAME_STR: &str = "CaffeineGuardOsd";
const TITLE: &str = "CaffeineGuard-OSD";

/// Thread message asking the OSD pump thread to re-read state and repaint.
const WM_OSD_UPDATE: u32 = WM_APP + 1;

// --- layout metrics (physical px) ---------------------------------------------

/// One icon cell (16x16).
pub(crate) const ICON: i32 = 16;
/// Space between the two icons in a row/column.
const GAP: i32 = 8;
/// Distance from the monitor edge.
const MARGIN: i32 = 12;
/// Crosshair half-extent (center position only).
const CROSS_ARM: i32 = 15;
/// Half of the crosshair's empty center.
const CROSS_GAP: i32 = 7;
/// Space between a crosshair arm tip and the nearest icon.
const CROSS_TO_ICON: i32 = 7;

// --- palette (COLORREF 0x00BBGGRR) --------------------------------------------

const COL_ACCENT: u32 = 0x003D_A3_E8; // #E8A33D app amber
const COL_LIGHT: u32 = 0x00ED_ED_ED; // #EDEDED
const COL_DARK: u32 = 0x0020_20_20; // #202020 contrast underlay
const COL_BROWN: u32 = 0x002B_4A_6B; // #6B4A2B cup steam
/// Sentinel background treated as fully transparent by the mask pass.
const MASK_COLOR: u32 = 0x00FF_00FF; // magenta

/// Everything the OSD needs to know; pushed by `manager::Core::emit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OsdState {
    pub enabled: bool,
    pub position: OsdPosition,
    /// Keep-awake active (coffee icon).
    pub awake: bool,
    /// Auto-blackout toggle on (eye-off icon).
    pub auto_blackout: bool,
    /// Blackout currently covering the screens (OSD hides).
    pub blackout: bool,
    /// Cursor-find feature enabled in settings (pointer icon, persistent
    /// like the coffee/eye icons — not tied to the transient effect).
    pub find: bool,
}

impl Default for OsdState {
    fn default() -> Self {
        OsdState {
            enabled: false,
            position: OsdPosition::TopRight,
            awake: false,
            auto_blackout: false,
            blackout: false,
            find: false,
        }
    }
}

impl OsdState {
    fn visible(&self) -> bool {
        self.enabled && (self.awake || self.auto_blackout || self.find) && !self.blackout
    }
}

// --- pure layout math (unit-tested) -------------------------------------------

/// Relative geometry inside the OSD window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Layout {
    pub w: i32,
    pub h: i32,
    pub coffee: Option<(i32, i32)>,
    pub eye: Option<(i32, i32)>,
    pub pointer: Option<(i32, i32)>,
    pub cross: Option<(i32, i32)>,
}

/// Icon arrangement for a position. Top/bottom rows run horizontally,
/// middle-left/right stack vertically, center draws a crosshair with the
/// coffee above and eye + pointer below it (slots reserved so the window
/// never resizes when a single feature toggles). Present icons fill slots
/// in order (coffee, eye, pointer) so a lone icon always sits inside the
/// span instead of at a fixed — possibly clipped — position.
pub(crate) fn layout(pos: OsdPosition, awake: bool, auto: bool, find: bool) -> Layout {
    let n = i32::from(awake) + i32::from(auto) + i32::from(find);
    // NOTE: i32::saturating_sub saturates at i32::MIN, not 0 — clamp here.
    let span = (n * ICON + (n - 1) * GAP).max(0);
    let step = ICON + GAP;
    let mut next = 0;
    let mut slot = |on: bool| -> Option<i32> {
        on.then(|| {
            let s = next;
            next += step;
            s
        })
    };
    let coffee_slot = slot(awake);
    let eye_slot = slot(auto);
    let pointer_slot = slot(find);
    match pos {
        OsdPosition::TopLeft
        | OsdPosition::TopCenter
        | OsdPosition::TopRight
        | OsdPosition::BottomLeft
        | OsdPosition::BottomCenter
        | OsdPosition::BottomRight => Layout {
            w: span,
            h: if n > 0 { ICON } else { 0 },
            coffee: coffee_slot.map(|s| (s, 0)),
            eye: eye_slot.map(|s| (s, 0)),
            pointer: pointer_slot.map(|s| (s, 0)),
            cross: None,
        },
        OsdPosition::MiddleLeft | OsdPosition::MiddleRight => Layout {
            w: if n > 0 { ICON } else { 0 },
            h: span,
            coffee: coffee_slot.map(|s| (0, s)),
            eye: eye_slot.map(|s| (0, s)),
            pointer: pointer_slot.map(|s| (0, s)),
            cross: None,
        },
        OsdPosition::Center => {
            let w = 2 * CROSS_ARM + 1;
            let top_block = CROSS_TO_ICON + ICON;
            let eye_y = top_block + 2 * CROSS_ARM + 1 + CROSS_TO_ICON;
            let pointer_y = eye_y + ICON + GAP;
            let h = pointer_y + ICON;
            Layout {
                w,
                h,
                coffee: awake.then_some(((w - ICON) / 2, 0)),
                eye: auto.then_some(((w - ICON) / 2, eye_y)),
                pointer: find.then_some(((w - ICON) / 2, pointer_y)),
                cross: Some((w / 2, top_block + CROSS_ARM)),
            }
        }
    }
}

/// Window origin for a layout size on `mon`.
pub(crate) fn place(mon: &MonitorRect, pos: OsdPosition, w: i32, h: i32) -> (i32, i32) {
    let x = match pos {
        OsdPosition::TopLeft | OsdPosition::MiddleLeft | OsdPosition::BottomLeft => {
            mon.x + MARGIN
        }
        OsdPosition::TopCenter | OsdPosition::Center | OsdPosition::BottomCenter => {
            mon.x + (mon.w - w) / 2
        }
        OsdPosition::TopRight | OsdPosition::MiddleRight | OsdPosition::BottomRight => {
            mon.x + mon.w - MARGIN - w
        }
    };
    let y = match pos {
        OsdPosition::TopLeft | OsdPosition::TopCenter | OsdPosition::TopRight => mon.y + MARGIN,
        OsdPosition::MiddleLeft | OsdPosition::Center | OsdPosition::MiddleRight => {
            mon.y + (mon.h - h) / 2
        }
        OsdPosition::BottomLeft | OsdPosition::BottomCenter | OsdPosition::BottomRight => {
            mon.y + mon.h - MARGIN - h
        }
    };
    (x, y)
}

// --- process-wide plumbing -----------------------------------------------------

static CLASS_NAME: OnceLock<&'static [u16]> = OnceLock::new();
static CLASS_ONCE: OnceLock<()> = OnceLock::new();
static STATE: OnceLock<Mutex<OsdState>> = OnceLock::new();
static HWND_CELL: OnceLock<Mutex<isize>> = OnceLock::new();
static THREAD_ID: OnceLock<Mutex<Option<u32>>> = OnceLock::new();
static THREAD_STARTED: AtomicBool = AtomicBool::new(false);

fn state_cell() -> &'static Mutex<OsdState> {
    STATE.get_or_init(|| Mutex::new(OsdState::default()))
}

fn hwnd_cell() -> &'static Mutex<isize> {
    HWND_CELL.get_or_init(|| Mutex::new(0))
}

fn thread_id_cell() -> &'static Mutex<Option<u32>> {
    THREAD_ID.get_or_init(|| Mutex::new(None))
}

fn class_name() -> &'static [u16] {
    CLASS_NAME.get_or_init(|| Box::leak(wide_null(CLASS_NAME_STR).into_boxed_slice()))
}

/// Push the latest OSD state. Cheap and callable from any thread: identical
/// states are coalesced, changes wake the pump thread with one message.
pub fn update(state: OsdState) {
    {
        let mut cur = state_cell().lock().unwrap();
        if *cur == state {
            return;
        }
        *cur = state;
    }
    if THREAD_STARTED.load(Ordering::SeqCst) {
        if let Ok(guard) = thread_id_cell().lock() {
            if let Some(tid) = *guard {
                post_thread_msg(tid, WM_OSD_UPDATE);
            }
        }
    } else if state.visible() {
        THREAD_STARTED.store(true, Ordering::SeqCst);
        std::thread::spawn(osd_thread);
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: isize,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    match msg {
        WM_DISPLAYCHANGE => {
            // Monitor geometry changed: re-anchor from the latest state.
            apply();
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

fn osd_thread() {
    enable_per_monitor_dpi();
    let instance = module_handle();
    ensure_class(instance);
    let mon = primary_monitor();
    let exstyle =
        (WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE)
            as u32;
    let title = wide_null(TITLE);
    let hwnd = create_window(
        exstyle,
        class_name(),
        &title,
        WS_POPUP, // hidden until first apply()
        mon.x,
        mon.y,
        1,
        1,
        instance,
    );
    if hwnd == 0 {
        crate::log::log_line("osd: window creation failed");
        thread_id_cell().lock().unwrap().take();
        THREAD_STARTED.store(false, Ordering::SeqCst);
        return;
    }
    apply_click_through(hwnd);
    let excluded = apply_capture_exclusion(hwnd);
    *hwnd_cell().lock().unwrap() = hwnd;
    *thread_id_cell().lock().unwrap() = Some(current_thread_id());
    crate::log::log_line(&format!("osd: hwnd={hwnd} capture_excluded={excluded}"));

    // Initial paint from whatever state arrived before/while starting; any
    // posts that raced us here are already covered by this apply().
    apply();

    unsafe {
        let mut msg: WinMsg = std::mem::zeroed();
        loop {
            let r = get_message(&mut msg);
            if r == 0 || r == -1 {
                break;
            }
            if msg.hwnd == 0 && msg.message == WM_OSD_UPDATE {
                apply();
                continue;
            }
            translate_message(&msg);
            dispatch_message(&msg);
        }
    }
    // Process exit path only: the window dies with the thread.
    *hwnd_cell().lock().unwrap() = 0;
    thread_id_cell().lock().unwrap().take();
}

/// Re-read the latest state and show/hide/move/repaint the OSD window.
/// Runs on the OSD pump thread only.
fn apply() {
    let hwnd = *hwnd_cell().lock().unwrap();
    if hwnd == 0 {
        return;
    }
    let st = *state_cell().lock().unwrap();
    if !st.visible() {
        hide_window(hwnd);
        return;
    }
    let lay = layout(st.position, st.awake, st.auto_blackout, st.find);
    let mon = primary_monitor();
    let (x, y) = place(&mon, st.position, lay.w, lay.h);
    let Some(surf) = create_argb_surface(lay.w, lay.h) else {
        return;
    };
    let rc = Rect {
        left: 0,
        top: 0,
        right: lay.w,
        bottom: lay.h,
    };
    fill_color(surf.dc, &rc, MASK_COLOR);
    draw_osd(surf.dc, &lay);
    finalize_argb(surf.bits, (lay.w * lay.h) as usize);
    if !update_layered_pixels(hwnd, x, y, &surf) {
        crate::log::log_line("osd: UpdateLayeredWindow failed");
    }
    show_noactivate(hwnd);
}

/// GDI writes leave alpha=0; convert the sentinel background to transparent
/// and everything else to opaque premultiplied ARGB (shapes are hard-edged,
/// so there is nothing in between).
fn finalize_argb(bits: *mut u32, n: usize) {
    unsafe {
        for i in 0..n {
            let p = bits.add(i);
            let v = *p & 0x00FF_FFFF;
            *p = if v == MASK_COLOR {
                0
            } else {
                0xFF00_0000 | v
            };
        }
    }
}

fn draw_osd(dc: isize, lay: &Layout) {
    if let Some((x, y)) = lay.coffee {
        draw_awake_icon(dc, x, y);
    }
    if let Some((x, y)) = lay.eye {
        draw_auto_icon(dc, x, y);
    }
    if let Some((x, y)) = lay.pointer {
        draw_find_icon(dc, x, y);
    }
    if let Some((cx, cy)) = lay.cross {
        draw_cross(dc, cx, cy);
    }
}

// --- icon glyphs (hard-edged GDI shapes) ---------------------------------------

/// Temporarily selects a pen and/or brush, restoring and deleting on drop.
struct Painter {
    dc: isize,
    old_pen: isize,
    old_brush: isize,
    pen: isize,
    brush: isize,
    owned_pen: bool,
    owned_brush: bool,
}

enum Pen {
    Null,
    Solid(i32, u32),
}

enum Brush {
    Null,
    Solid(u32),
}

impl Painter {
    fn new(dc: isize, pen: Pen, brush: Brush) -> Self {
        let (pen, owned_pen) = match pen {
            Pen::Null => (stock_obj(NULL_PEN), false),
            Pen::Solid(w, c) => (create_pen(w, c), true),
        };
        let (brush, owned_brush) = match brush {
            Brush::Null => (stock_obj(NULL_BRUSH), false),
            Brush::Solid(c) => (create_solid_brush(c), true),
        };
        let old_pen = select_object(dc, pen);
        let old_brush = select_object(dc, brush);
        Painter {
            dc,
            old_pen,
            old_brush,
            pen,
            brush,
            owned_pen,
            owned_brush,
        }
    }
}

impl Drop for Painter {
    fn drop(&mut self) {
        select_object(self.dc, self.old_pen);
        select_object(self.dc, self.old_brush);
        // Stock objects must not be deleted.
        if self.owned_pen {
            delete_object(self.pen);
        }
        if self.owned_brush {
            delete_object(self.brush);
        }
    }
}

/// Coffee cup = keep-awake active.
fn draw_awake_icon(dc: isize, x: i32, y: i32) {
    {
        let _p = Painter::new(dc, Pen::Solid(1, COL_BROWN), Brush::Null);
        move_to(dc, x + 5, y + 1);
        line_to(dc, x + 5, y + 3);
        move_to(dc, x + 8, y + 1);
        line_to(dc, x + 8, y + 3);
    }
    {
        let _p = Painter::new(dc, Pen::Null, Brush::Solid(COL_ACCENT));
        round_rect_shape(dc, x + 2, y + 4, x + 11, y + 13, 2);
    }
    {
        let _p = Painter::new(dc, Pen::Solid(1, COL_ACCENT), Brush::Null);
        ellipse_shape(dc, x + 10, y + 6, x + 15, y + 11);
    }
}

/// Crossed-out eye = auto-blackout armed.
fn draw_auto_icon(dc: isize, x: i32, y: i32) {
    // Dark 3px underlays keep the glyph readable on light backgrounds.
    {
        let _p = Painter::new(dc, Pen::Solid(3, COL_DARK), Brush::Null);
        ellipse_shape(dc, x + 1, y + 5, x + 15, y + 11);
        move_to(dc, x, y + 15);
        line_to(dc, x + 15, y);
    }
    {
        let _p = Painter::new(dc, Pen::Solid(1, COL_LIGHT), Brush::Null);
        ellipse_shape(dc, x + 1, y + 5, x + 15, y + 11);
        move_to(dc, x, y + 15);
        line_to(dc, x + 15, y);
    }
    {
        let _p = Painter::new(dc, Pen::Null, Brush::Solid(COL_LIGHT));
        ellipse_shape(dc, x + 6, y + 7, x + 10, y + 9);
    }
}

/// Crosshair reticle for the center position: 4 arms around an empty middle.
fn draw_cross(dc: isize, cx: i32, cy: i32) {
    for &(width, color) in &[(3i32, COL_DARK), (1, COL_ACCENT)] {
        let _p = Painter::new(dc, Pen::Solid(width, color), Brush::Null);
        move_to(dc, cx - CROSS_ARM, cy);
        line_to(dc, cx - CROSS_GAP, cy);
        move_to(dc, cx + CROSS_GAP, cy);
        line_to(dc, cx + CROSS_ARM, cy);
        move_to(dc, cx, cy - CROSS_ARM);
        line_to(dc, cx, cy - CROSS_GAP);
        move_to(dc, cx, cy + CROSS_GAP);
        line_to(dc, cx, cy + CROSS_ARM);
    }
}

/// Pointer arrow = cursor-find effect playing. Light fill with a dark rim,
/// echoing the magnified cursor the effect itself draws.
fn draw_find_icon(dc: isize, x: i32, y: i32) {
    // Classic arrow in a ~9x13 box (tip at the origin), so the 1px rim and
    // the +1 light offset still land inside the 16x16 cell.
    const BASE: [(i32, i32); 7] = [
        (0, 0),
        (0, 11),
        (2, 8),
        (5, 13),
        (7, 12),
        (5, 8),
        (9, 8),
    ];
    let dark: Vec<Point> = BASE
        .iter()
        .map(|&(px, py)| Point { x: x + px + 2, y: y + py + 1 })
        .collect();
    let light: Vec<Point> = BASE
        .iter()
        .map(|&(px, py)| Point { x: x + px + 3, y: y + py + 2 })
        .collect();
    {
        let _p = Painter::new(dc, Pen::Solid(2, COL_DARK), Brush::Solid(COL_DARK));
        polygon_shape(dc, &dark);
    }
    {
        let _p = Painter::new(dc, Pen::Solid(1, COL_LIGHT), Brush::Solid(COL_LIGHT));
        polygon_shape(dc, &light);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::OsdPosition::*;
    use std::time::Duration;

    fn mon_1080p() -> MonitorRect {
        MonitorRect {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
            primary: true,
        }
    }

    #[test]
    fn horizontal_rows_lay_icons_side_by_side() {
        for pos in [TopLeft, TopCenter, TopRight, BottomLeft, BottomCenter, BottomRight] {
            let l = layout(pos, true, true, true);
            assert_eq!(l.w, 3 * ICON + 2 * GAP, "{pos:?} width");
            assert_eq!(l.h, ICON, "{pos:?} height");
            assert_eq!(l.coffee, Some((0, 0)), "{pos:?} coffee");
            assert_eq!(l.eye, Some((ICON + GAP, 0)), "{pos:?} eye");
            assert_eq!(l.pointer, Some((2 * (ICON + GAP), 0)), "{pos:?} pointer");
            assert_eq!(l.cross, None, "{pos:?} cross");
        }
    }

    #[test]
    fn middle_edges_stack_vertically() {
        for pos in [MiddleLeft, MiddleRight] {
            let l = layout(pos, true, true, true);
            assert_eq!(l.w, ICON, "{pos:?} width");
            assert_eq!(l.h, 3 * ICON + 2 * GAP, "{pos:?} height");
            assert_eq!(l.coffee, Some((0, 0)), "{pos:?} coffee");
            assert_eq!(l.eye, Some((0, ICON + GAP)), "{pos:?} eye");
            assert_eq!(l.pointer, Some((0, 2 * (ICON + GAP))), "{pos:?} pointer");
        }
    }

    #[test]
    fn center_is_crosshair_with_icons_above_and_below() {
        let l = layout(Center, true, true, true);
        assert_eq!(l.w, 2 * CROSS_ARM + 1);
        let top_block = CROSS_TO_ICON + ICON;
        let eye_y = top_block + 2 * CROSS_ARM + 1 + CROSS_TO_ICON;
        let pointer_y = eye_y + ICON + GAP;
        assert_eq!(l.h, pointer_y + ICON);
        assert_eq!(l.cross, Some((l.w / 2, top_block + CROSS_ARM)));
        assert_eq!(l.coffee, Some(((l.w - ICON) / 2, 0)));
        assert_eq!(l.eye, Some(((l.w - ICON) / 2, eye_y)));
        assert_eq!(l.pointer, Some(((l.w - ICON) / 2, pointer_y)));
        // Fewer features keep the same window (stable geometry): all slots
        // are reserved even when nothing is drawn in them.
        let one = layout(Center, true, false, false);
        assert_eq!((one.w, one.h), (l.w, l.h));
        assert_eq!(one.eye, None);
        assert_eq!(one.pointer, None);
    }

    #[test]
    fn single_icon_shrinks_span() {
        let l = layout(TopRight, true, false, false);
        assert_eq!((l.w, l.h), (ICON, ICON));
        let find_only = layout(TopRight, false, false, true);
        assert_eq!((find_only.w, find_only.h), (ICON, ICON));
        assert_eq!(find_only.coffee, None);
        assert_eq!(find_only.pointer, Some((0, 0)));
        let none = layout(TopRight, false, false, false);
        assert_eq!((none.w, none.h), (0, 0));
        assert_eq!(none.coffee, None);
        assert_eq!(none.eye, None);
        assert_eq!(none.pointer, None);
    }

    #[test]
    fn nine_positions_anchor_correctly() {
        let m = mon_1080p();
        let (w, h) = (2 * ICON + GAP, ICON); // 40x16 horizontal span
        assert_eq!(place(&m, TopLeft, w, h), (MARGIN, MARGIN));
        assert_eq!(place(&m, TopCenter, w, h), ((1920 - w) / 2, MARGIN));
        assert_eq!(place(&m, TopRight, w, h), (1920 - MARGIN - w, MARGIN));
        let (vw, vh) = (ICON, 2 * ICON + GAP); // vertical span
        assert_eq!(place(&m, MiddleLeft, vw, vh), (MARGIN, (1080 - vh) / 2));
        assert_eq!(place(&m, MiddleRight, vw, vh), (1920 - MARGIN - vw, (1080 - vh) / 2));
        assert_eq!(place(&m, BottomCenter, w, h), ((1920 - w) / 2, 1080 - MARGIN - h));
        assert_eq!(place(&m, BottomLeft, w, h), (MARGIN, 1080 - MARGIN - h));
        assert_eq!(place(&m, BottomRight, w, h), (1920 - MARGIN - w, 1080 - MARGIN - h));
        // Multi-monitor offset.
        let m2 = MonitorRect {
            x: 1920,
            y: 200,
            w: 2560,
            h: 1440,
            primary: false,
        };
        assert_eq!(place(&m2, TopLeft, w, h), (1920 + MARGIN, 200 + MARGIN));
    }

    #[test]
    fn visibility_rules() {
        let base = OsdState {
            enabled: true,
            position: TopRight,
            awake: true,
            auto_blackout: true,
            blackout: false,
            find: false,
        };
        assert!(base.visible());
        assert!(!OsdState { enabled: false, ..base }.visible(), "OSD off");
        assert!(
            !OsdState {
                awake: false,
                auto_blackout: false,
                find: false,
                ..base
            }
            .visible(),
            "no feature on"
        );
        assert!(!OsdState { blackout: true, ..base }.visible(), "during blackout");
        assert!(OsdState { awake: false, ..base }.visible(), "auto only");
        assert!(OsdState { auto_blackout: false, ..base }.visible(), "awake only");
        // The cursor-find toggle alone still lights the OSD pointer icon —
        // same "feature armed" semantics as the coffee and eye icons.
        assert!(
            OsdState {
                awake: false,
                auto_blackout: false,
                find: true,
                ..base
            }
            .visible(),
            "find only"
        );
    }

    #[test]
    fn argb_mask_pass() {
        let mut px: [u32; 3] = [0x00FF_00FF, 0x003D_A3_E8, 0x0020_2020];
        finalize_argb(px.as_mut_ptr(), 3);
        assert_eq!(px[0], 0, "sentinel -> transparent");
        assert_eq!(px[1], 0xFF3D_A3_E8, "shape -> opaque");
        assert_eq!(px[2], 0xFF20_2020, "dark colors stay opaque");
    }

    /// Find-only OSD: a lone pointer icon must render (exercises the GDI
    /// polygon path) inside a one-icon span on the real desktop.
    #[test]
    fn osd_shows_find_icon_alone() {
        let _g = crate::testsupport::window_test_lock();
        enable_per_monitor_dpi();
        let mut sized = false;
        for _ in 0..40 {
            // Re-assert each poll: sibling OSD tests overwrite the shared
            // state cell, so a passive wait would race them.
            update(OsdState {
                enabled: true,
                position: TopRight,
                awake: false,
                auto_blackout: false,
                blackout: false,
                find: true,
            });
            std::thread::sleep(Duration::from_millis(100));
            let hwnd = find_window_by_title(TITLE);
            if hwnd != 0 && is_window_visible(hwnd) {
                if let Some(rc) = window_rect(hwnd) {
                    // The layered window covers exactly the one-icon span.
                    if rc.right - rc.left == ICON && rc.bottom - rc.top == ICON {
                        sized = true;
                        break;
                    }
                }
            }
        }
        assert!(sized, "find-only OSD shows a single pointer-sized icon");
        // Leave the desktop clean.
        update(OsdState {
            enabled: false,
            ..OsdState::default()
        });
        for _ in 0..30 {
            std::thread::sleep(Duration::from_millis(100));
            let hwnd = find_window_by_title(TITLE);
            if hwnd == 0 || !is_window_visible(hwnd) {
                break;
            }
        }
    }

    /// The find icon must actually paint pixels (the window sizing tests
    /// cannot catch a silently failing GDI draw).
    #[test]
    fn find_icon_draws_pixels() {
        let Some(surf) = create_argb_surface(ICON, ICON) else {
            panic!("surface");
        };
        let rc = Rect {
            left: 0,
            top: 0,
            right: ICON,
            bottom: ICON,
        };
        fill_color(surf.dc, &rc, MASK_COLOR);
        draw_find_icon(surf.dc, 0, 0);
        finalize_argb(surf.bits, (ICON * ICON) as usize);
        let (mut opaque, mut light_px, mut dark_px) = (0, 0, 0);
        for i in 0..(ICON * ICON) as usize {
            let v = unsafe { *surf.bits.add(i) };
            if v >> 24 != 0 {
                opaque += 1;
                let (r, g, b) = ((v >> 16) & 0xFF, (v >> 8) & 0xFF, v & 0xFF);
                if r > 200 && g > 200 && b > 200 {
                    light_px += 1;
                }
                if r < 80 && g < 80 && b < 80 {
                    dark_px += 1;
                }
            }
        }
        assert!(
            opaque > 20,
            "pointer icon must paint pixels, got {opaque} opaque"
        );
        assert!(light_px > 5, "white fill expected, got {light_px}");
        assert!(dark_px > 5, "dark rim expected, got {dark_px}");
    }

    /// Real desktop smoke test: shows, hides during blackout, position change.
    #[test]
    fn osd_window_show_hide_and_move() {
        let _g = crate::testsupport::window_test_lock();
        // Match the OSD thread's DPI context: without this, Win32 virtualizes
        // this thread's GetWindowRect into logical pixels (e.g. 40px -> 32px
        // at 125% scaling) while the OSD places itself in physical pixels.
        enable_per_monitor_dpi();
        let base = OsdState {
            enabled: true,
            position: TopRight,
            awake: true,
            auto_blackout: true,
            blackout: false,
            find: false,
        };
        update(base);

        let mut hwnd = 0;
        for _ in 0..40 {
            std::thread::sleep(Duration::from_millis(100));
            hwnd = find_window_by_title(TITLE);
            if hwnd != 0 && is_window_visible(hwnd) {
                break;
            }
        }
        assert_ne!(hwnd, 0, "osd window created");

        // Anchored at top-right of the primary monitor with margin. Re-assert
        // the state each poll: concurrent tests (e.g. the cursor-find effect
        // emitting the *real* settings state) can overwrite the shared OSD
        // cell at any moment.
        let m = primary_monitor();
        let (w, h) = (2 * ICON + GAP, ICON);
        let mut anchored = false;
        for _ in 0..30 {
            update(base);
            std::thread::sleep(Duration::from_millis(100));
            if let Some(rc) = window_rect(hwnd) {
                if rc.right - rc.left == w
                    && rc.bottom - rc.top == h
                    && rc.right == m.x + m.w - MARGIN
                    && rc.top == m.y + MARGIN
                {
                    anchored = true;
                    break;
                }
            }
        }
        assert!(anchored, "osd anchored at top-right");

        // Blackout showing -> hidden (but window survives for reuse).
        update(OsdState { blackout: true, ..base });
        let mut hidden = false;
        for _ in 0..30 {
            std::thread::sleep(Duration::from_millis(100));
            if !is_window_visible(hwnd) {
                hidden = true;
                break;
            }
        }
        assert!(hidden, "osd hides during blackout");
        assert_eq!(find_window_by_title(TITLE), hwnd, "window kept for reuse");

        // Back visible and re-anchored after moving to bottom-left.
        update(OsdState {
            position: BottomLeft,
            ..base
        });
        let mut moved = false;
        for _ in 0..30 {
            std::thread::sleep(Duration::from_millis(100));
            if is_window_visible(hwnd) {
                let rc = window_rect(hwnd).expect("osd rect");
                if rc.left == m.x + MARGIN && rc.bottom == m.y + m.h - MARGIN {
                    moved = true;
                    break;
                }
            }
        }
        assert!(moved, "osd re-anchored to bottom-left");

        // Leave the desktop clean.
        update(OsdState { enabled: false, ..base });
        for _ in 0..30 {
            std::thread::sleep(Duration::from_millis(100));
            if !is_window_visible(hwnd) {
                break;
            }
        }
    }
}
