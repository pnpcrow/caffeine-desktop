//! Shake-to-find-the-cursor: fast side-to-side shake detection plus the
//! transient effect windows that make the pointer pop.
//!
//! Detection reuses the global LL mouse hook stream (`unlock::InputEvent`).
//! While the blackout is NOT showing, every mouse move feeds a
//! [`FindDetector`]; a deliberate oscillation (several direction reversals
//! with real amplitude inside a short window) fires the effect. During a
//! blackout the same gesture stays an *unlock* action, so the two consumers
//! are mutually exclusive.
//!
//! The effect follows the OSD's window philosophy — raw Win32 layered
//! windows, click-through, `WDA_EXCLUDEFROMCAPTURE`, topmost, no
//! activation — but unlike the OSD the pixels are rasterized per-frame on
//! the CPU (alpha-faded ripples cannot be expressed with hard-edged GDI
//! shapes) and pushed with `UpdateLayeredWindow` at ~60fps:
//!
//! * one *spotlight* window that tracks the cursor: a magnified classic
//!   arrow (scale follows the measured shake amplitude) and/or expanding
//!   circular ripples — two independently toggleable effects;
//! * one *arrow* window centered on every monitor that does NOT hold the
//!   cursor, showing a big solid arrow pointing toward the monitor that
//!   does (optional).
//!
//! The whole effect is human-eyes-only and never blocks input.

use crate::win32::*;
use std::collections::VecDeque;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// --- detection tunables --------------------------------------------------------

/// Trailing window for the find-shake (shorter than the unlock shake:
/// finding the cursor should fire almost immediately).
pub const FIND_WINDOW_MS: u128 = 550;
/// Minimum horizontal excursion (px) for a segment to count as a stroke.
pub const FIND_MIN_DX: i32 = 20;
/// Direction reversals required inside the window.
pub const FIND_FLIPS_NEEDED: u32 = 3;
/// Minimum total pointer path (px) inside the window.
pub const FIND_MIN_PATH: i64 = 220;

// --- effect tunables -----------------------------------------------------------

/// How long the effect keeps playing after the last detected shake.
const HOLD_MS: u64 = 1600;
/// Render cadence.
const FRAME_MS: u64 = 16;
/// Spotlight window edge (px). Must be >= 2 * RING_R1.
const SPOT_SIZE: i32 = 380;
/// Ripple radius range (px from the cursor).
const RING_R0: f32 = 22.0;
const RING_R1: f32 = 165.0;
/// Ripple lifetime and spawn interval.
const RING_LIFE_MS: u64 = 900;
const RING_SPAWN_MS: u64 = 420;
/// Soft warm glow radius under the cursor (px).
const GLOW_R: f32 = 68.0;
/// Cursor magnification bounds (multiple of the base 12x19 arrow).
const SCALE_MIN: f32 = 1.7;
const SCALE_MAX: f32 = 3.4;
/// Amplitude (px) that maps to the magnification ceiling.
const SCALE_AMP_FULL: f32 = 190.0;
/// Arrow-hint geometry: total length as a fraction of monitor height,
/// clamped — big and readable from across the desk.
const ARROW_MON_FRACTION: f32 = 0.30;
const ARROW_LEN_MIN: f32 = 240.0;
const ARROW_LEN_MAX: f32 = 560.0;
/// Head width and shaft thickness as fractions of the arrow length.
const ARROW_HEAD_W: f32 = 0.52;
const ARROW_SHAFT_T: f32 = 0.24;
/// Monitor topology refresh interval.
const RECONCILE_MS: u64 = 500;
/// Fade-to-transparent ramp at the very end of the effect.
const FADE_MS: u64 = 250;

/// Amber accent (#E8A33D), straight RGB channels.
const AMBER: (f32, f32, f32) = (232.0, 163.0, 61.0);

const CLASS_NAME_STR: &str = "CaffeineGuardFind";
const SPOT_TITLE: &str = "CaffeineGuard-Find-Spot";
const ARROW_TITLE: &str = "CaffeineGuard-Find-Arrow";

// --- pure detection ------------------------------------------------------------

/// Stateful side-to-side shake detector. Returns the horizontal amplitude
/// (peak-to-peak, px) of the trailing window at the moment the shake
/// threshold is crossed; that amplitude drives the cursor magnification.
#[derive(Debug, Default)]
pub struct FindDetector {
    trail: VecDeque<(u128, i32, i32)>,
}

impl FindDetector {
    pub fn new() -> Self {
        FindDetector {
            trail: VecDeque::new(),
        }
    }

    pub fn reset(&mut self) {
        self.trail.clear();
    }

    /// Feed a mouse position. `Some(amplitude_px)` on a detected shake.
    pub fn feed(&mut self, x: i32, y: i32) -> Option<i32> {
        let now = now_ms();
        self.trail.push_back((now, x, y));
        while let Some(&(t, _, _)) = self.trail.front() {
            if now.saturating_sub(t) > FIND_WINDOW_MS {
                self.trail.pop_front();
            } else {
                break;
            }
        }

        let mut flips: u32 = 0;
        let mut last_sign: i32 = 0;
        let mut path: i64 = 0;
        let mut min_x = x;
        let mut max_x = x;
        let pts: Vec<(i32, i32)> = self.trail.iter().map(|&(_, px, py)| (px, py)).collect();
        for w in pts.windows(2) {
            let dx = w[1].0 - w[0].0;
            let dy = w[1].1 - w[0].1;
            path += (dx.abs() as i64) + (dy.abs() as i64);
            min_x = min_x.min(w[1].0);
            max_x = max_x.max(w[1].0);
            if dx.abs() >= FIND_MIN_DX {
                let s = if dx > 0 { 1 } else { -1 };
                if last_sign != 0 && s != last_sign {
                    flips += 1;
                }
                last_sign = s;
            }
        }
        if flips >= FIND_FLIPS_NEEDED && path >= FIND_MIN_PATH {
            // Re-arm from scratch: while the user keeps shaking, the next
            // trigger (and its fresh amplitude) is only a few strokes away.
            self.trail.clear();
            Some(max_x - min_x)
        } else {
            None
        }
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

// --- pure geometry --------------------------------------------------------------

/// Distance from point P to segment AB.
pub(crate) fn seg_dist(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let abx = bx - ax;
    let aby = by - ay;
    let len2 = abx * abx + aby * aby;
    if len2 <= f32::EPSILON {
        return ((px - ax).powi(2) + (py - ay).powi(2)).sqrt();
    }
    let t = (((px - ax) * abx + (py - ay) * aby) / len2).clamp(0.0, 1.0);
    let cx = ax + t * abx;
    let cy = ay + t * aby;
    ((px - cx).powi(2) + (py - cy).powi(2)).sqrt()
}

/// Classic left-tilted pointer arrow, tip at (0,0), base extent ~12x19,
/// scaled by `scale`. Drawn so the *tip* sits exactly on the real cursor's
/// hotspot — the OS keeps painting the true (small) arrow on top, so the
/// magnified copy reads as "the cursor grew".
pub(crate) fn arrow_poly(scale: f32) -> [(f32, f32); 7] {
    const BASE: [(f32, f32); 7] = [
        (0.0, 0.0),
        (0.0, 14.0),
        (3.2, 10.8),
        (5.8, 16.6),
        (8.2, 15.6),
        (5.7, 9.9),
        (10.6, 9.9),
    ];
    BASE.map(|(x, y)| (x * scale, y * scale))
}

/// Signed distance from a point to a closed polygon (positive inside).
/// Containment via even-odd crossing; the distance is to the nearest edge.
/// Coverage/AA values are derived from it as `0.5 + sd / 1.5` clamped, so
/// the sd itself must be returned unclamped (round-tripping a saturated
/// coverage value would collapse the whole interior onto the rim).
pub(crate) fn poly_signed_dist(px: f32, py: f32, poly: &[(f32, f32)]) -> f32 {
    let mut inside = false;
    let mut best = f32::MAX;
    let n = poly.len();
    for i in 0..n {
        let (ax, ay) = poly[i];
        let (bx, by) = poly[(i + 1) % n];
        if (ay > py) != (by > py) {
            let t = (py - ay) / (by - ay);
            let x_at = ax + t * (bx - ax);
            if x_at > px {
                inside = !inside;
            }
        }
        best = best.min(seg_dist(px, py, ax, ay, bx, by));
    }
    if inside {
        best
    } else {
        -best
    }
}

/// Snap a monitor-to-monitor vector to the nearest of 8 directions and
/// return its unit vector (screen coords, +y = down).
pub(crate) fn direction_8way(dx: i32, dy: i32) -> (f32, f32) {
    let ang = (dy as f32).atan2(dx as f32);
    let oct = (ang / (std::f32::consts::PI / 4.0)).round();
    let ang8 = oct * (std::f32::consts::PI / 4.0);
    (ang8.cos(), ang8.sin())
}

/// Map a measured shake amplitude (px) to the cursor magnification.
pub(crate) fn amplitude_to_scale(amp: i32) -> f32 {
    (SCALE_MIN + amp as f32 / SCALE_AMP_FULL * (SCALE_MAX - SCALE_MIN)).clamp(SCALE_MIN, SCALE_MAX)
}

// --- pure rasterizers -----------------------------------------------------------

/// One ripple, with its geometry precomputed for the frame being drawn.
struct RingSpec {
    r: f32,
    width: f32,
    alpha: f32,
}

/// Rasterize one spotlight frame into `px` (w*h premultiplied ARGB u32s).
/// Center of the window == the cursor hotspot. `magnify` draws the enlarged
/// cursor arrow; `ripple` enables the glow + the `rings` layer (callers
/// pass an empty slice when it is off).
fn render_spotlight(
    px: &mut [u32],
    w: i32,
    h: i32,
    scale: f32,
    rings: &[RingSpec],
    master: f32,
    magnify: bool,
    ripple: bool,
) {
    let cx = w as f32 / 2.0;
    let cy = h as f32 / 2.0;
    let poly = arrow_poly(scale);
    let outline_w = (0.9 * scale).max(1.2);
    // Bounding box of the magnified arrow: only pixels near it need the
    // (comparatively expensive) polygon test.
    let mut bx0 = f32::MAX;
    let mut by0 = f32::MAX;
    let mut bx1 = f32::MIN;
    let mut by1 = f32::MIN;
    for &(x, y) in &poly {
        bx0 = bx0.min(x);
        by0 = by0.min(y);
        bx1 = bx1.max(x);
        by1 = by1.max(y);
    }
    let (bx0, by0, bx1, by1) = (
        (cx + bx0 - 2.0).floor().max(0.0) as i32,
        (cy + by0 - 2.0).floor().max(0.0) as i32,
        (cx + bx1 + 2.0).ceil().min(w as f32) as i32,
        (cy + by1 + 2.0).ceil().min(h as f32) as i32,
    );

    for y in 0..h {
        for x in 0..w {
            let fx = x as f32 + 0.5 - cx;
            let fy = y as f32 + 0.5 - cy;
            let d = (fx * fx + fy * fy).sqrt();

            // Amber base layer: glow + ripples, additive alpha.
            let mut a_layer = 0.0f32;
            if ripple && d < GLOW_R {
                let k = 1.0 - d / GLOW_R;
                a_layer += k * k * 0.30;
            }
            for ring in rings {
                let edge = (1.0 - (d - ring.r).abs() / ring.width).clamp(0.0, 1.0);
                a_layer += edge * ring.alpha;
            }
            a_layer = a_layer.min(1.0) * master;

            // Premultiplied amber background.
            let (mut pr, mut pg, mut pb, mut pa) = (
                AMBER.0 * a_layer,
                AMBER.1 * a_layer,
                AMBER.2 * a_layer,
                a_layer,
            );

            // Magnified arrow (white fill, black outline) over the base.
            if magnify && x >= bx0 && x < bx1 && y >= by0 && y < by1 {
                let lpx = x as f32 + 0.5 - cx;
                let lpy = y as f32 + 0.5 - cy;
                let sd = poly_signed_dist(lpx, lpy, &poly);
                if sd > -2.0 {
                    let cov_full = (0.5 + sd / 2.0).clamp(0.0, 1.0);
                    let cov_fill = (0.5 + (sd - outline_w) / 2.0).clamp(0.0, 1.0);
                    let a_full = cov_full * master;
                    let a_fill = cov_fill * master;
                    let keep = 1.0 - pa;
                    pr += 255.0 * a_fill * keep;
                    pg += 255.0 * a_fill * keep;
                    pb += 255.0 * a_fill * keep;
                    // The black outline contributes alpha only (rgb stays 0).
                    pa += keep * a_full;
                }
            }

            let i = (y * w + x) as usize;
            px[i] = pack_premult(pr, pg, pb, pa);
        }
    }
}

fn pack_premult(r: f32, g: f32, b: f32, a: f32) -> u32 {
    let a = a.clamp(0.0, 1.0);
    let r = r.clamp(0.0, a * 255.0);
    let g = g.clamp(0.0, a * 255.0);
    let b = b.clamp(0.0, a * 255.0);
    ((a * 255.0).round() as u32) << 24
        | ((r).round() as u32) << 16
        | ((g).round() as u32) << 8
        | (b).round() as u32
}

/// Window side (px) and arrow length for a monitor-hint arrow, derived
/// from the monitor height so the hint reads at any resolution.
pub(crate) fn arrow_geometry(mon_h: i32) -> (i32, f32) {
    let l = (mon_h as f32 * ARROW_MON_FRACTION).clamp(ARROW_LEN_MIN, ARROW_LEN_MAX);
    // The rotated bounding box is largest at 45°: (l + head_w) / sqrt(2).
    let side = ((l * (1.0 + ARROW_HEAD_W)) * std::f32::consts::FRAC_1_SQRT_2).ceil() as i32 + 12;
    (side, l)
}

/// Solid arrow polygon (7-gon: rectangular shaft + triangular head),
/// pointing along the unit vector `dir`, centered at the window's middle,
/// scaled by the pulse factor. Coordinates are window-local.
fn arrow_body(side: i32, dir: (f32, f32), l: f32, pulse: f32) -> [(f32, f32); 7] {
    let k = 0.95 + 0.10 * pulse;
    let len = l * k;
    let hw = l * ARROW_HEAD_W * k;
    let st = l * ARROW_SHAFT_T * k;
    let head_len = len * 0.30;
    let half = len / 2.0;
    let mut pts = [(0.0f32, 0.0f32); 7];
    let local = [
        (-half, -st / 2.0),
        (half - head_len, -st / 2.0),
        (half - head_len, -hw / 2.0),
        (half, 0.0),
        (half - head_len, hw / 2.0),
        (half - head_len, st / 2.0),
        (-half, st / 2.0),
    ];
    let c = side as f32 / 2.0;
    for (i, &(x, y)) in local.iter().enumerate() {
        pts[i] = (c + x * dir.0 - y * dir.1, c + x * dir.1 + y * dir.0);
    }
    pts
}

/// Rasterize one monitor-hint frame: a big solid arrow (white body, dark
/// rim) pointing along `dir`, gently pulsing in size and opacity.
fn render_arrow(px: &mut [u32], side: i32, dir: (f32, f32), l: f32, pulse: f32, master: f32) {
    let poly = arrow_body(side, dir, l, pulse);
    let outline_w = (l * 0.03).max(5.0);
    let alpha_w = (0.86 + 0.12 * pulse) * master;

    for y in 0..side {
        for x in 0..side {
            let sd = poly_signed_dist(x as f32 + 0.5, y as f32 + 0.5, &poly);
            let mut out = 0u32;
            if sd > -2.0 {
                let cov_full = (0.5 + sd / 2.0).clamp(0.0, 1.0);
                let cov_fill = (0.5 + (sd - outline_w) / 2.0).clamp(0.0, 1.0);
                let a_full = cov_full * alpha_w;
                let a_fill = cov_fill * alpha_w;
                out = pack_premult(255.0 * a_fill, 255.0 * a_fill, 255.0 * a_fill, a_full);
            }
            let i = (y * side + x) as usize;
            px[i] = out;
        }
    }
}

// --- process-wide controller -----------------------------------------------------

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

/// Which sub-effects a trigger runs; snapshotted from the settings at
/// trigger time so mid-effect setting changes apply on the next trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FindOpts {
    /// Enlarge the cursor itself (scales with shake strength).
    pub magnify: bool,
    /// Circular ripple waves around the cursor.
    pub ripple: bool,
    /// Solid direction arrows on the monitors without the cursor.
    pub arrows: bool,
}

impl Default for FindOpts {
    fn default() -> Self {
        FindOpts {
            magnify: true,
            ripple: true,
            arrows: true,
        }
    }
}

static ACTIVE: AtomicBool = AtomicBool::new(false);
static CURSOR_X: AtomicI32 = AtomicI32::new(0);
static CURSOR_Y: AtomicI32 = AtomicI32::new(0);
static AMPLITUDE: AtomicI32 = AtomicI32::new(0);
static DEADLINE: AtomicU64 = AtomicU64::new(0);
static THREAD_STARTED: AtomicBool = AtomicBool::new(false);
static OPTS: Mutex<FindOpts> = Mutex::new(FindOpts {
    magnify: true,
    ripple: true,
    arrows: true,
});

static CLASS_NAME: OnceLock<&'static [u16]> = OnceLock::new();
static CLASS_ONCE: OnceLock<()> = OnceLock::new();

fn class_name() -> &'static [u16] {
    CLASS_NAME.get_or_init(|| Box::leak(wide_null(CLASS_NAME_STR).into_boxed_slice()))
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

unsafe extern "system" fn wnd_proc(
    hwnd: isize,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    def_window_proc(hwnd, msg, wparam, lparam)
}

/// Fire (or re-fire) the find effect at the cursor. `amplitude` is the
/// measured shake amplitude in px (drives magnification); `opts` selects
/// which sub-effects run.
pub fn trigger(x: i32, y: i32, amplitude: i32, opts: FindOpts) {
    CURSOR_X.store(x, Ordering::SeqCst);
    CURSOR_Y.store(y, Ordering::SeqCst);
    AMPLITUDE.store(amplitude, Ordering::SeqCst);
    if let Ok(mut o) = OPTS.lock() {
        *o = opts;
    }
    DEADLINE.store(tick_ms() as u64 + HOLD_MS, Ordering::SeqCst);
    ACTIVE.store(true, Ordering::SeqCst);
    if !THREAD_STARTED.swap(true, Ordering::SeqCst) {
        std::thread::spawn(find_thread);
    }
}

/// Track the cursor while the effect plays (called for every move).
pub fn notify_move(x: i32, y: i32) {
    CURSOR_X.store(x, Ordering::SeqCst);
    CURSOR_Y.store(y, Ordering::SeqCst);
}

/// Stop the effect at the next frame (feature disabled or blackout start).
pub fn cancel() {
    DEADLINE.store(tick_ms() as u64, Ordering::SeqCst);
}

pub fn is_showing() -> bool {
    ACTIVE.load(Ordering::SeqCst)
}

// --- effect thread ----------------------------------------------------------------

struct ArrowWin {
    mon_idx: usize,
    hwnd: isize,
    dir: (f32, f32),
    side: i32,
    length: f32,
    surf: ArgbSurface,
}

struct Effect {
    started: u64,
    spot: isize,
    spot_surf: ArgbSurface,
    arrows: Vec<ArrowWin>,
    monitors: Vec<MonitorRect>,
    cursor_mon: usize,
    rings: Vec<u64>,
    next_ring: u64,
    scale: f32,
}

fn create_effect_window(title: &str, w: i32, h: i32, instance: isize) -> isize {
    let exstyle =
        (WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE)
            as u32;
    let wide = wide_null(title);
    let hwnd = create_window(
        exstyle,
        class_name(),
        &wide,
        WS_POPUP, // hidden until the first frame is pushed
        0,
        0,
        w,
        h,
        instance,
    );
    if hwnd != 0 {
        apply_capture_exclusion(hwnd);
        apply_click_through(hwnd);
    }
    hwnd
}

impl Effect {
    fn create(instance: isize, now: u64) -> Option<Effect> {
        let spot_surf = create_argb_surface(SPOT_SIZE, SPOT_SIZE)?;
        let spot = create_effect_window(SPOT_TITLE, SPOT_SIZE, SPOT_SIZE, instance);
        if spot == 0 {
            return None;
        }
        let mut fx = Effect {
            started: now,
            spot,
            spot_surf,
            arrows: Vec::new(),
            monitors: Vec::new(),
            cursor_mon: 0,
            rings: Vec::new(),
            next_ring: now,
            scale: amplitude_to_scale(AMPLITUDE.load(Ordering::SeqCst)),
        };
        fx.reconcile(instance);
        Some(fx)
    }

    /// Refresh monitor topology and the arrow-window set: one hint window on
    /// every monitor that does not hold the cursor, pointing at the one that
    /// does. No-ops (and removes the windows) when the setting is off.
    fn reconcile(&mut self, instance: isize) {
        self.monitors = enum_monitors();
        if self.monitors.is_empty() {
            return;
        }
        let (cx, cy) = (
            CURSOR_X.load(Ordering::SeqCst),
            CURSOR_Y.load(Ordering::SeqCst),
        );
        self.cursor_mon = self
            .monitors
            .iter()
            .position(|m| {
                cx >= m.x && cx < m.x + m.w && cy >= m.y && cy < m.y + m.h
            })
            .unwrap_or(0);

        let opts = *OPTS.lock().unwrap();
        let want_arrows = opts.arrows && self.monitors.len() > 1;
        if !want_arrows {
            self.destroy_arrows();
            return;
        }

        // Drop hints whose monitor now holds the cursor or vanished.
        self.arrows.retain(|a| {
            let gone = a.mon_idx >= self.monitors.len() || a.mon_idx == self.cursor_mon;
            if gone {
                destroy_window(a.hwnd);
            }
            !gone
        });
        // Recompute directions (topology may have shifted) and fill gaps.
        for i in 0..self.monitors.len() {
            if i == self.cursor_mon {
                continue;
            }
            let (dx, dy) = {
                let from = &self.monitors[i];
                let to = &self.monitors[self.cursor_mon];
                (
                    to.x + to.w / 2 - (from.x + from.w / 2),
                    to.y + to.h / 2 - (from.y + from.h / 2),
                )
            };
            let dir = direction_8way(dx, dy);
            match self.arrows.iter_mut().find(|a| a.mon_idx == i) {
                Some(a) => a.dir = dir,
                None => {
                    let (side, length) = arrow_geometry(self.monitors[i].h);
                    let hwnd = create_effect_window(
                        &format!("{ARROW_TITLE}-{i}"),
                        side,
                        side,
                        instance,
                    );
                    if hwnd != 0 {
                        match create_argb_surface(side, side) {
                            Some(surf) => self.arrows.push(ArrowWin {
                                mon_idx: i,
                                hwnd,
                                dir,
                                side,
                                length,
                                surf,
                            }),
                            None => destroy_window(hwnd),
                        }
                    }
                }
            }
        }
    }

    fn destroy_arrows(&mut self) {
        for a in &self.arrows {
            destroy_window(a.hwnd);
        }
        self.arrows.clear();
    }

    fn render(&mut self, now: u64) {
        if self.monitors.is_empty() {
            return;
        }
        let opts = *OPTS.lock().unwrap();
        // Smooth toward the latest measured amplitude.
        if opts.magnify {
            let target = amplitude_to_scale(AMPLITUDE.load(Ordering::SeqCst));
            self.scale += (target - self.scale) * 0.12;
        }

        let deadline = DEADLINE.load(Ordering::SeqCst);
        let master = if deadline > now {
            ((deadline - now) as f32 / FADE_MS as f32).min(1.0)
        } else {
            0.0
        };

        // Ripple bookkeeping.
        self.rings.retain(|&t| now.saturating_sub(t) < RING_LIFE_MS);
        while self.next_ring <= now && deadline.saturating_sub(self.next_ring) > 300 {
            self.rings.push(self.next_ring);
            self.next_ring += RING_SPAWN_MS;
        }
        let rings: Vec<RingSpec> = if opts.ripple {
            self.rings
                .iter()
                .map(|&t| {
                    let prog = (now.saturating_sub(t) as f32 / RING_LIFE_MS as f32).min(1.0);
                    let eased = 1.0 - (1.0 - prog) * (1.0 - prog); // ease-out
                    RingSpec {
                        r: RING_R0 + (RING_R1 - RING_R0) * eased,
                        width: 2.6 + 1.6 * (1.0 - prog),
                        alpha: (1.0 - prog).powi(2) * 0.85,
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        // Spotlight follows the cursor.
        let (cx, cy) = (
            CURSOR_X.load(Ordering::SeqCst),
            CURSOR_Y.load(Ordering::SeqCst),
        );
        render_spotlight(
            unsafe {
                std::slice::from_raw_parts_mut(self.spot_surf.bits, (SPOT_SIZE * SPOT_SIZE) as usize)
            },
            SPOT_SIZE,
            SPOT_SIZE,
            self.scale,
            &rings,
            master,
            opts.magnify,
            opts.ripple,
        );
        update_layered_pixels(
            self.spot,
            cx - SPOT_SIZE / 2,
            cy - SPOT_SIZE / 2,
            &self.spot_surf,
        );
        show_noactivate(self.spot);

        // Cursor may have crossed into another monitor: re-aim immediately.
        let crossed = !point_in_monitor(&self.monitors[self.cursor_mon], cx, cy);
        if crossed {
            self.reconcile(module_handle());
        }

        // Arrow hints pulse in place at each monitor's center.
        let phase = ((now - self.started) % 1300) as f32 / 1300.0;
        let pulse = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * phase).cos();
        for a in &self.arrows {
            render_arrow(
                unsafe {
                    std::slice::from_raw_parts_mut(a.surf.bits, (a.side * a.side) as usize)
                },
                a.side,
                a.dir,
                a.length,
                pulse,
                master,
            );
            let mon = &self.monitors[a.mon_idx];
            update_layered_pixels(
                a.hwnd,
                mon.x + (mon.w - a.side) / 2,
                mon.y + (mon.h - a.side) / 2,
                &a.surf,
            );
            show_noactivate(a.hwnd);
        }
    }
}

fn point_in_monitor(m: &MonitorRect, x: i32, y: i32) -> bool {
    x >= m.x && x < m.x + m.w && y >= m.y && y < m.y + m.h
}

impl Drop for Effect {
    fn drop(&mut self) {
        destroy_window(self.spot);
        self.destroy_arrows();
    }
}

/// Parked-until-triggered render loop. Created once per process; while the
/// effect is idle it wakes at ~60Hz only to poll the ACTIVE flag.
fn find_thread() {
    enable_per_monitor_dpi();
    let instance = module_handle();
    ensure_class(instance);

    let mut fx: Option<Effect> = None;
    let mut next_frame: u64 = 0;
    let mut next_reconcile: u64 = 0;
    loop {
        std::thread::sleep(Duration::from_millis(FRAME_MS));
        let now = tick_ms() as u64;
        if fx.is_none() {
            if ACTIVE.load(Ordering::SeqCst) {
                fx = Effect::create(instance, now);
                match fx {
                    Some(_) => {}
                    None => {
                        ACTIVE.store(false, Ordering::SeqCst);
                        continue;
                    }
                }
                next_frame = now;
                next_reconcile = now + RECONCILE_MS;
            }
            continue;
        }
        if now >= DEADLINE.load(Ordering::SeqCst) {
            ACTIVE.store(false, Ordering::SeqCst);
            if now < DEADLINE.load(Ordering::SeqCst) {
                // A trigger raced the shutdown: honor it and keep playing.
                ACTIVE.store(true, Ordering::SeqCst);
            } else {
                fx = None; // Drop destroys the windows.
                continue;
            }
        }
        let f = fx.as_mut().expect("effect is live here");
        if now >= next_reconcile {
            f.reconcile(instance);
            next_reconcile = now + RECONCILE_MS;
        }
        if now >= next_frame {
            f.render(now);
            next_frame = now + FRAME_MS;
        }
    }
}

// --- tests -------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // detection -------------------------------------------------------------

    /// Deliberate side-to-side shake: `width`-px strokes, alternating.
    /// Returns absolute points starting from 500 so each scenario can use a
    /// fresh detector without teleport jumps between sections.
    fn shake_path(width: i32, strokes: u32) -> Vec<(i32, i32)> {
        let mut pts = Vec::new();
        let mut x = 500;
        let mut dir = 1;
        for _ in 0..strokes {
            for _ in 0..3 {
                x += dir * (width / 3).max(1);
                pts.push((x, 500));
            }
            dir = -dir;
        }
        pts
    }

    #[test]
    fn shake_triggers_with_amplitude() {
        let mut d = FindDetector::new();
        let mut amp = None;
        for &(x, y) in &shake_path(70, 8) {
            if let Some(a) = d.feed(x, y) {
                amp = Some(a);
                break;
            }
        }
        let amp = amp.expect("a real shake must trigger");
        // Peak-to-peak of a ±70px oscillation is at least the stroke width
        // (the trigger fires as soon as the 3rd reversal lands).
        assert!(amp >= 60, "amplitude {amp} should reflect the stroke width");
    }

    #[test]
    fn amplitude_tracks_stroke_width() {
        let measure = |width: i32| {
            let mut d = FindDetector::new();
            let mut a = 0;
            for &(x, y) in &shake_path(width, 8) {
                if let Some(v) = d.feed(x, y) {
                    a = v;
                }
            }
            a
        };
        let narrow = measure(30);
        let wide = measure(200);
        assert!(wide > narrow, "wider shakes must read as stronger");
        assert!(wide >= 180, "a 200px stroke spans ~200px peak-to-peak");
    }

    #[test]
    fn straight_drift_and_slow_meander_do_not_trigger() {
        // Fresh detectors per scenario: a shared trail would string the
        // scenarios together with phantom teleport strokes.
        let mut d = FindDetector::new();
        for i in 0..60 {
            assert!(d.feed(i * 30, 500).is_none(), "straight drag");
        }

        let mut d = FindDetector::new();
        let mut x = 400;
        for _ in 0..40 {
            x += 8;
            assert!(d.feed(x, 500).is_none(), "tiny jitters");
            x -= 8;
            assert!(d.feed(x, 500).is_none(), "tiny jitters");
        }

        let mut d = FindDetector::new();
        for &(x, y) in &shake_path(90, 2) {
            assert!(d.feed(x, y).is_none(), "too few reversals");
        }
    }

    #[test]
    fn vertical_movement_does_not_trigger() {
        let mut d = FindDetector::new();
        let mut y = 300;
        let mut dir = 1;
        for _ in 0..10 {
            for _ in 0..4 {
                y += dir * 40;
                assert!(d.feed(600, y).is_none(), "vertical shake is not a find gesture");
            }
            dir = -dir;
        }
    }

    // geometry ----------------------------------------------------------------

    #[test]
    fn segment_distance_basics() {
        let d = seg_dist(5.0, 5.0, 0.0, 0.0, 10.0, 0.0);
        assert!((d - 5.0).abs() < 1e-4, "point above segment");
        let d = seg_dist(5.0, 0.0, 0.0, 0.0, 10.0, 0.0);
        assert!(d.abs() < 1e-4, "point on segment");
        let d = seg_dist(-4.0, 3.0, 0.0, 0.0, 10.0, 0.0);
        assert!((d - 5.0).abs() < 1e-4, "beyond the A end caps to A");
        let d = seg_dist(5.0, 2.0, 5.0, 5.0, 5.0, 5.0);
        assert!((d - 3.0).abs() < 1e-4, "degenerate segment = point distance");
    }

    #[test]
    fn arrow_polygon_coverage() {
        let poly = arrow_poly(3.0);
        let cov = |x: f32, y: f32| (0.5 + poly_signed_dist(x, y, &poly) / 1.5).clamp(0.0, 1.0);
        // Deep interior of the head (below the hypotenuse, left of the stem).
        assert!(cov(4.5, 18.0) > 0.99, "inside");
        // Far outside.
        assert_eq!(cov(60.0, 60.0), 0.0, "outside");
        // Left of the straight left edge (x=0): outside.
        assert_eq!(cov(-3.0, 21.0), 0.0, "left of the edge");
        // Just inside the left edge: partial coverage (AA ramp).
        let edge = cov(0.5, 21.0);
        assert!(
            edge > 0.5 && edge < 1.0,
            "aa ramp straddles the edge, got {edge}"
        );
    }

    #[test]
    fn directions_snap_to_eight_ways() {
        let (x, y) = direction_8way(2240, 180);
        assert!((x - 1.0).abs() < 1e-4 && y.abs() < 1e-4, "east: {x},{y}");
        let (x, y) = direction_8way(-2240, 180);
        assert!((x + 1.0).abs() < 1e-4 && y.abs() < 1e-4, "west");
        let (x, y) = direction_8way(0, 1080);
        assert!(x.abs() < 1e-4 && (y - 1.0).abs() < 1e-4, "south (+y is down)");
        let (x, y) = direction_8way(0, -1080);
        assert!(x.abs() < 1e-4 && (y + 1.0).abs() < 1e-4, "north");
        let (x, y) = direction_8way(1920, -1080);
        let d = std::f32::consts::FRAC_1_SQRT_2;
        assert!((x - d).abs() < 1e-3 && (y + d).abs() < 1e-3, "north-east");
        let (x, y) = direction_8way(-1920, 1080);
        assert!((x + d).abs() < 1e-3 && (y - d).abs() < 1e-3, "south-west");
    }

    #[test]
    fn amplitude_maps_to_bounded_scale() {
        assert!((amplitude_to_scale(0) - SCALE_MIN).abs() < 1e-4);
        assert!((amplitude_to_scale(1000) - SCALE_MAX).abs() < 1e-4);
        let s = amplitude_to_scale(95);
        assert!((SCALE_MIN..SCALE_MAX).contains(&s));
        assert!(s > SCALE_MIN, "mid amplitudes magnify beyond the floor");
    }

    // rasterizers ---------------------------------------------------------------

    #[test]
    fn spotlight_frame_is_valid_premultiplied_argb() {
        let w = SPOT_SIZE;
        let mut px = vec![0u32; (w * w) as usize];
        let rings = [RingSpec {
            r: 100.0,
            width: 3.0,
            alpha: 0.6,
        }];
        render_spotlight(&mut px, w, w, 2.5, &rings, 1.0, true, true);
        for v in &px {
            let a = v >> 24;
            let r = (v >> 16) & 0xFF;
            let g = (v >> 8) & 0xFF;
            let b = v & 0xFF;
            assert!(r <= a && g <= a && b <= a, "premultiplied: {v:#010x}");
        }
        // Corner is far from every layer: fully transparent.
        assert_eq!(px[0], 0, "corner transparent");
        // A pixel on the ripple ring carries amber, not white.
        let c = (w / 2) as usize;
        let ring_px = px[c * w as usize + (c + 100)];
        assert!(ring_px != 0, "ring visible at r=100");
        assert!(((ring_px >> 16) & 0xFF) > ((ring_px >> 8) & 0xFF), "amber tint");
        // Inside the magnified arrow the fill is near-white and opaque.
        let ax = c + (3.0 * 2.5) as usize;
        let ay = c + (5.0 * 2.5) as usize;
        let arrow_px = px[ay * w as usize + ax];
        assert!((arrow_px >> 24) > 200, "arrow opaque: {arrow_px:#010x}");
        assert!(((arrow_px >> 16) & 0xFF) > 200, "arrow white fill");
    }

    #[test]
    fn spotlight_respects_effect_toggles() {
        let w = SPOT_SIZE;
        let c = (w / 2) as usize;
        let rings = [RingSpec {
            r: 100.0,
            width: 3.0,
            alpha: 0.6,
        }];
        // Magnify off: the arrow's spot may still carry the amber glow, but
        // there must be no opaque white arrow body on top of it.
        let mut px = vec![0u32; (w * w) as usize];
        render_spotlight(&mut px, w, w, 2.5, &rings, 1.0, false, true);
        let ax = c + (3.0 * 2.5) as usize;
        let ay = c + (5.0 * 2.5) as usize;
        let v = px[ay * w as usize + ax];
        assert!(
            (v >> 24) < 100 && ((v >> 16) & 0xFF) < 180,
            "no white arrow when magnify off, got {v:#010x}"
        );
        // Ripple off (no glow, no rings): only the arrow remains.
        let mut px = vec![0u32; (w * w) as usize];
        render_spotlight(&mut px, w, w, 2.5, &[], 1.0, true, false);
        assert_eq!(px[c * w as usize + (c + 100)] >> 24, 0, "no ring layer");
        // Probe 60px above center: inside the glow radius but well outside
        // the (magnify-on) arrow polygon, so only the glow could paint it.
        let glow = px[(c - 60) * w as usize + c];
        assert_eq!(glow >> 24, 0, "no glow when ripple off");
        let ax = c + (3.0 * 2.5) as usize;
        let ay = c + (5.0 * 2.5) as usize;
        assert!(
            (px[ay * w as usize + ax] >> 24) > 200,
            "arrow still drawn when ripple off"
        );
    }

    #[test]
    fn arrow_geometry_scales_with_monitor() {
        let (s_1080, l_1080) = arrow_geometry(1080);
        assert!((l_1080 - 324.0).abs() < 1.0, "30% of 1080p height");
        // The window must fit the arrow rotated to any of the 8 directions.
        let hw = l_1080 * ARROW_HEAD_W * 1.05; // max pulse scale
        let need = (l_1080 + hw) * std::f32::consts::FRAC_1_SQRT_2;
        assert!(
            s_1080 as f32 >= need,
            "side {s_1080} must cover the rotated bbox {need}"
        );
        // Clamped at the ceiling on 4K, at the floor on tiny screens.
        let (_, l_4k) = arrow_geometry(2160);
        assert!((l_4k - ARROW_LEN_MAX).abs() < 1.0);
        let (_, l_small) = arrow_geometry(400);
        assert!((l_small - ARROW_LEN_MIN).abs() < 1.0);
    }

    #[test]
    fn arrow_frame_points_and_pulses() {
        let (side, l) = arrow_geometry(1080);
        let c = (side / 2) as usize;
        let at = |px: &[u32], x: usize, y: usize| px[y * side as usize + x];
        // Pointing east: tip near (c + l/2, c), mid-shaft near (c - l/4, c).
        let mut px = vec![0u32; (side * side) as usize];
        render_arrow(&mut px, side, (1.0, 0.0), l, 0.5, 1.0);
        let tip_x = (c as f32 + l * 0.47) as usize;
        assert!(at(&px, tip_x, c) >> 24 > 150, "near the tip");
        let shaft_x = (c as f32 - l * 0.25) as usize;
        let shaft_px = at(&px, shaft_x, c);
        assert!(shaft_px >> 24 > 150, "mid-shaft solid");
        assert!(
            ((shaft_px >> 16) & 0xFF) > 200,
            "shaft body is white, got {shaft_px:#010x}"
        );
        assert_eq!(at(&px, 10, 10), 0, "far off the arrow");
        // Below the shaft but inside the head span: head wing is filled.
        let wing_y = (c as f32 + l * 0.15) as usize;
        let wing_x = (c as f32 + l * 0.30) as usize;
        assert!(at(&px, wing_x, wing_y) >> 24 > 100, "head wing filled");
        // master=0 fades everything out.
        render_arrow(&mut px, side, (1.0, 0.0), l, 0.5, 0.0);
        assert_eq!(at(&px, shaft_x, c), 0, "master fade to nothing");
    }

    // real windows --------------------------------------------------------------

    #[test]
    fn effect_windows_appear_and_expire() {
        let _g = crate::testsupport::window_test_lock();
        enable_per_monitor_dpi();
        let m = primary_monitor();
        let start = tick_ms() as u64;
        trigger(
            m.x + m.w / 2,
            m.y + m.h / 2,
            140,
            FindOpts {
                arrows: false,
                ..FindOpts::default()
            },
        );

        // Spotlight window appears, tracking the cursor position.
        let mut hwnd = 0;
        for _ in 0..40 {
            std::thread::sleep(Duration::from_millis(50));
            hwnd = find_window_by_title(SPOT_TITLE);
            if hwnd != 0 && is_window_visible(hwnd) {
                break;
            }
        }
        assert_ne!(hwnd, 0, "spotlight window created");
        assert!(is_showing());

        // Re-trigger refreshes position and extends the deadline.
        std::thread::sleep(Duration::from_millis(150));
        trigger(
            m.x + m.w / 4,
            m.y + m.h / 2,
            200,
            FindOpts {
                arrows: false,
                ..FindOpts::default()
            },
        );
        let mut moved = false;
        for _ in 0..30 {
            std::thread::sleep(Duration::from_millis(50));
            if let Some(rc) = window_rect(hwnd) {
                if rc.left <= m.x + m.w / 4 && rc.right >= m.x + m.w / 4 {
                    moved = true;
                    break;
                }
            }
        }
        assert!(moved, "spotlight follows the cursor");

        // Arrows disabled in this trigger: none may exist.
        assert_eq!(find_window_by_title(&format!("{ARROW_TITLE}-0")), 0);

        // After the (extended) hold expires everything is gone.
        let hard_deadline = start + 2 * HOLD_MS + 3000;
        let mut gone = false;
        while (tick_ms() as u64) < hard_deadline {
            std::thread::sleep(Duration::from_millis(100));
            if !is_showing() && find_window_by_title(SPOT_TITLE) == 0 {
                gone = true;
                break;
            }
        }
        assert!(gone, "effect must end after the hold");
    }
}
