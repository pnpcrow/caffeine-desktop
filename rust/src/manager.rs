//! Process-lifetime core: owns settings, overlay, hooks, watchers.
//!
//! The Flutter GUI calls into [`api`] (flutter_rust_bridge). All long-lived
//! threads run here; UI updates flow back through `StreamSink<AppEvent>`.
//! Nothing here blocks the caller's thread: FRB executes these functions on
//! its own worker pool, and change notifications are push-based.

use crate::overlay::OverlayManager;
use crate::settings::{Settings, UnlockMouse};
use crate::unlock::{self, InputEvent, ShakeTracker};
use crate::frb_generated::StreamSink;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone)]
pub struct Status {
    pub awake: bool,
    pub blackout: bool,
    pub auto_blackout: bool,
    pub auto_blackout_secs: u64,
    pub version: String,
}

pub struct Core {
    settings: Mutex<Settings>,
    awake_on: AtomicBool,
    overlay: OverlayManager,
    sinks: Mutex<Vec<StreamSink<Status>>>,
}

static CORE: OnceLock<Core> = OnceLock::new();

pub fn core() -> &'static Core {
    CORE.get_or_init(|| {
        let settings = Settings::load();
        let awake = settings.awake_enabled;
        Core {
            settings: Mutex::new(settings),
            awake_on: AtomicBool::new(awake),
            overlay: OverlayManager::new(),
            sinks: Mutex::new(Vec::new()),
        }
    })
}

impl Core {
    pub(crate) fn status(&self) -> Status {
        let s = self.settings.lock().unwrap();
        Status {
            awake: self.awake_on.load(Ordering::SeqCst),
            blackout: self.overlay.is_showing(),
            auto_blackout: s.auto_blackout_enabled,
            auto_blackout_secs: s.auto_blackout_secs,
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    pub(crate) fn emit(&self) {
        let ev = self.status();
        let (osd_enabled, osd_position) = {
            let s = self.settings.lock().unwrap();
            (s.osd_enabled, s.osd_position)
        };
        crate::osd::update(crate::osd::OsdState {
            enabled: osd_enabled,
            position: osd_position,
            awake: ev.awake,
            auto_blackout: ev.auto_blackout,
            blackout: ev.blackout,
            find: crate::cursor_find::is_showing(),
        });
        if let Ok(mut sinks) = self.sinks.lock() {
            sinks.retain(|s| s.add(ev.clone()).is_ok());
        }
    }

    pub(crate) fn apply_awake(&self, on: bool) {
        self.awake_on.store(on, Ordering::SeqCst);
        crate::awake::apply_awake(on);
    }

    pub(crate) fn update_settings(&self, s: Settings) {
        *self.settings.lock().unwrap() = s;
    }

    pub(crate) fn settings_snapshot(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    pub(crate) fn set_awake_flag(&self, on: bool) {
        self.settings.lock().unwrap().awake_enabled = on;
        self.apply_awake(on);
        let _ = self.settings.lock().unwrap().save();
    }

    pub(crate) fn set_auto_flag(&self, on: bool) {
        self.settings.lock().unwrap().auto_blackout_enabled = on;
        let _ = self.settings.lock().unwrap().save();
    }

    pub(crate) fn overlay_show(&self) {
        // A blackout replaces the find effect; the shake gesture goes back
        // to meaning "unlock" while the overlay is up.
        crate::cursor_find::cancel();
        self.overlay.show();
    }

    pub(crate) fn overlay_hide(&self) {
        self.overlay.hide();
    }
}

/// One-time startup: apply persisted state and launch background threads.
/// Safe to call once (subsequent calls only register an extra event sink
/// via `watch_events`, they do not duplicate threads).
pub fn init_core() {
    static STARTED: AtomicBool = AtomicBool::new(false);
    crate::log::init_file_log();
    crate::log::log_line(&format!("init_core (v{})", env!("CARGO_PKG_VERSION")));
    let c = core();
    c.apply_awake(c.awake_on.load(Ordering::SeqCst));
    // Sync the OSD once at startup (before any sink exists) so the indicator
    // reflects persisted settings immediately, not on the first UI action.
    c.emit();
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }

    // Consumer: input hooks -> unlock decisions + cursor-find detection.
    let rx = unlock::install_hooks();
    std::thread::spawn(move || {
        let mut tracker = ShakeTracker::new();
        let mut armed = false;
        let mut find = crate::cursor_find::FindDetector::new();
        while let Ok(ev) = rx.recv() {
            let c = core();
            if !c.overlay.is_showing() {
                if armed {
                    armed = false;
                    tracker.reset();
                }
                // While no blackout covers the screens, a fast side-to-side
                // shake is a *find my cursor* gesture instead of an unlock.
                if let InputEvent::Move(x, y) = ev {
                    let (enabled, arrows) = {
                        let s = c.settings.lock().unwrap();
                        (s.cursor_find_enabled, s.cursor_find_arrows)
                    };
                    if enabled {
                        match find.feed(x, y) {
                            Some(amplitude) => {
                                crate::log::log_line(&format!(
                                    "find-cursor gesture (amplitude {amplitude}px)"
                                ));
                                crate::cursor_find::trigger(x, y, amplitude, arrows);
                            }
                            None => crate::cursor_find::notify_move(x, y),
                        }
                    } else if crate::cursor_find::is_showing() {
                        crate::cursor_find::cancel();
                    }
                }
                continue;
            }
            if !armed {
                armed = true;
                tracker.reset();
            }
            let s = c.settings.lock().unwrap().clone();
            let dismiss = match ev {
                InputEvent::Key(vk) => unlock::key_unlocks(s.unlock_key, vk),
                InputEvent::Move(x, y) => tracker.feed(s.unlock_mouse, x, y),
                InputEvent::Click => s.unlock_mouse == UnlockMouse::Click,
            };
            if dismiss {
                crate::log::log_line("unlock gesture accepted");
                c.overlay.hide();
                c.emit();
            }
        }
    });

    // Idle watcher with startup grace (don't black out right after launch
    // when the session was already idle longer than the timeout).
    std::thread::spawn(move || {
        const GRACE_SECS: u64 = 60;
        let started = std::time::Instant::now();
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            if started.elapsed() < std::time::Duration::from_secs(GRACE_SECS) {
                continue;
            }
            let c = core();
            let (enabled, threshold_ms) = {
                let s = c.settings.lock().unwrap();
                (
                    s.auto_blackout_enabled,
                    s.auto_blackout_secs.saturating_mul(1000),
                )
            };
            if enabled
                && threshold_ms > 0
                && !c.overlay.is_showing()
                && crate::idle::idle_ms() >= threshold_ms
            {
                crate::log::log_line("idle timeout reached, auto blackout");
                c.overlay.show();
                c.emit();
            }
        }
    });

    // Keep-awake re-assertion lives in awake.rs's owner thread: it is the only
    // thread allowed to touch SetThreadExecutionState, so asserts and releases
    // can never end up on different threads.
}

/// Subscribe to status changes. The current status is pushed immediately.
pub fn watch_events(sink: StreamSink<Status>) {
    let c = core();
    let _ = sink.add(c.status());
    if let Ok(mut sinks) = c.sinks.lock() {
        sinks.push(sink);
    }
}
