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
//!   arrow (scale follows the measured shake amplitude) over expanding
//!   circular ripples;
//! * one *arrow* window centered on every monitor that does NOT hold the
//!   cursor, pointing toward the monitor that does (optional).
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
/// Arrow-hint window edge (px), centered on each foreign monitor.
const ARROW_SIZE: i32 = 240;
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
/// Center of the window == the cursor hotspot.
fn render_spotlight(px: &mut [u32], w: i32, h: i32, scale: f32, rings: &[RingSpec], master: f32) {
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
            if d < GLOW_R {
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
            if x >= bx0 && x < bx1 && y >= by0 && y < by1 {
                let lpx = x as f32 + 0.5 - cx;
                let lpy = y as f32 + 0.5 - cy;
                let sd = poly_signed_dist(lpx, lpy, &poly);
                if sd > -1.5 {
                    let cov_full = (0.5 + sd / 1.5).clamp(0.0, 1.0);
                    let cov_fill = (0.5 + (sd - outline_w) / 1.5).clamp(0.0, 1.0);
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

/// Rasterize one arrow-hint frame: a thick chevron pointing along the unit
/// vector `dir`, pulsing with `pulse` (0..1), faded by `master`.
fn render_arrow(px: &mut [u32], w: i32, h: i32, dir: (f32, f32), pulse: f32, master: f32) {
    let c = (w as f32 / 2.0, h as f32 / 2.0);
    let (ux, uy) = dir;
    let (pxv, pyv) = (-uy, ux); // perpendicular
    let l = 62.0 * (0.94 + 0.10 * pulse);
    let half = 46.0 * (0.94 + 0.10 * pulse);
    let thick = 15.0;
    let tip = (c.0 + ux * l, c.1 + uy * l);
    let a1 = (c.0 - ux * l * 0.5 + pxv * half, c.1 - uy * l * 0.5 + pyv * half);
    let a2 = (c.0 - ux * l * 0.5 - pxv * half, c.1 - uy * l * 0.5 - pyv * half);
    let alpha_w = (185.0 + 60.0 * pulse) / 255.0 * master;

    for y in 0..h {
        for x in 0..w {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            let d1 = seg_dist(fx, fy, tip.0, tip.1, a1.0, a1.1);
            let d2 = seg_dist(fx, fy, tip.0, tip.1, a2.0, a2.1);
            let d = d1.min(d2);
            // White chevron with a soft dark halo around it.
            let a_white = (0.5 + (thick / 2.0 - d) / 1.5).clamp(0.0, 1.0) * alpha_w;
            let a_halo = (0.5 + (thick / 2.0 + 4.0 - d) / 2.5)
                .clamp(0.0, 1.0)
                .min(1.0)
                * 0.75
                * master
                * (1.0 - a_white);
            let pa = (a_white + a_halo).min(1.0);
            let pr = 255.0 * a_white;
            let pg = 255.0 * a_white;
            let pb = 255.0 * a_white;
            let i = (y * w + x) as usize;
            px[i] = pack_premult(pr, pg, pb, pa);
        }
    }
}

// --- process-wide controller -----------------------------------------------------

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use std::sync::OnceLock;

static ACTIVE: AtomicBool = AtomicBool::new(false);
static CURSOR_X: AtomicI32 = AtomicI32::new(0);
static CURSOR_Y: AtomicI32 = AtomicI32::new(0);
static AMPLITUDE: AtomicI32 = AtomicI32::new(0);
static DEADLINE: AtomicU64 = AtomicU64::new(0);
static ARROWS_ON: AtomicBool = AtomicBool::new(false);
static THREAD_STARTED: AtomicBool = AtomicBool::new(false);

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
/// measured shake amplitude in px (drives magnification); `arrows` is the
/// current arrow-hint setting snapshot.
pub fn trigger(x: i32, y: i32, amplitude: i32, arrows: bool) {
    CURSOR_X.store(x, Ordering::SeqCst);
    CURSOR_Y.store(y, Ordering::SeqCst);
    AMPLITUDE.store(amplitude, Ordering::SeqCst);
    ARROWS_ON.store(arrows, Ordering::SeqCst);
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
}

struct Effect {
    started: u64,
    spot: isize,
    spot_surf: ArgbSurface,
    arrow_surf: ArgbSurface,
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
        let arrow_surf = create_argb_surface(ARROW_SIZE, ARROW_SIZE)?;
        let spot = create_effect_window(SPOT_TITLE, SPOT_SIZE, SPOT_SIZE, instance);
        if spot == 0 {
            return None;
        }
        let mut fx = Effect {
            started: now,
            spot,
            spot_surf,
            arrow_surf,
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

        let want_arrows = ARROWS_ON.load(Ordering::SeqCst) && self.monitors.len() > 1;
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
                    let hwnd = create_effect_window(
                        &format!("{ARROW_TITLE}-{i}"),
                        ARROW_SIZE,
                        ARROW_SIZE,
                        instance,
                    );
                    if hwnd != 0 {
                        self.arrows.push(ArrowWin {
                            mon_idx: i,
                            hwnd,
                            dir,
                        });
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
        // Smooth toward the latest measured amplitude.
        let target = amplitude_to_scale(AMPLITUDE.load(Ordering::SeqCst));
        self.scale += (target - self.scale) * 0.12;

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
        let rings: Vec<RingSpec> = self
            .rings
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
            .collect();

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
                    std::slice::from_raw_parts_mut(
                        self.arrow_surf.bits,
                        (ARROW_SIZE * ARROW_SIZE) as usize,
                    )
                },
                ARROW_SIZE,
                ARROW_SIZE,
                a.dir,
                pulse,
                master,
            );
            let mon = &self.monitors[a.mon_idx];
            update_layered_pixels(
                a.hwnd,
                mon.x + (mon.w - ARROW_SIZE) / 2,
                mon.y + (mon.h - ARROW_SIZE) / 2,
                &self.arrow_surf,
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
                    Some(_) => crate::manager::core().emit(),
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
                crate::manager::core().emit();
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
        render_spotlight(&mut px, w, w, 2.5, &rings, 1.0);
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
    fn arrow_frame_points_and_pulses() {
        let w = ARROW_SIZE;
        let c = (w / 2) as usize;
        // Pointing east: tip at (c+62, c), arms back to (c-31, c±46).
        let at = |px: &[u32], x: usize, y: usize| px[y * w as usize + x];
        let mut px = vec![0u32; (w * w) as usize];
        render_arrow(&mut px, w, w, (1.0, 0.0), 0.5, 1.0);
        assert!(at(&px, c + 62, c) >> 24 > 150, "tip on the chevron");
        assert!(at(&px, c + 15, c + 23) >> 24 > 150, "lower arm midpoint");
        assert!(at(&px, c + 15, c - 23) >> 24 > 150, "upper arm midpoint");
        assert_eq!(at(&px, 10, 10), 0, "far off the chevron");
        // master=0 fades everything out.
        render_arrow(&mut px, w, w, (1.0, 0.0), 0.5, 0.0);
        assert_eq!(at(&px, c + 62, c), 0, "master fade to nothing");
    }

    // real windows --------------------------------------------------------------

    #[test]
    fn effect_windows_appear_and_expire() {
        enable_per_monitor_dpi();
        let m = primary_monitor();
        let start = tick_ms() as u64;
        trigger(m.x + m.w / 2, m.y + m.h / 2, 140, false);

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
        trigger(m.x + m.w / 4, m.y + m.h / 2, 200, false);
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
