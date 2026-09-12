//! Display/system keep-awake via SetThreadExecutionState.
//!
//! While asserted, Windows will not turn the display off, sleep, or start the
//! screensaver — the "caffeine" core of the app.
//!
//! SetThreadExecutionState(ES_CONTINUOUS) is *per-thread*: an assert made on
//! one thread cannot be released from another, and it evaporates when the
//! asserting thread exits. Flutter calls arrive on arbitrary flutter_rust_bridge
//! worker threads, so every transition is funneled to a single dedicated owner
//! thread which is also the only place the periodic re-assert happens.

use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use windows::Win32::System::Power::{
    SetThreadExecutionState, ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED,
};

/// Re-assert interval while awake (some drivers/power tools clear the state).
const REASSERT_INTERVAL: Duration = Duration::from_secs(30);

struct Cmd {
    on: bool,
    /// Ack channel; apply_awake waits on it so state changes are durable
    /// before the caller returns (quit path depends on this).
    ack: Option<Sender<()>>,
}

static SENDER: OnceLock<Mutex<Sender<Cmd>>> = OnceLock::new();

fn sender() -> &'static Mutex<Sender<Cmd>> {
    SENDER.get_or_init(|| {
        let (tx, rx) = channel::<Cmd>();
        std::thread::Builder::new()
            .name("caffeine-awake".into())
            .spawn(move || owner(rx))
            .expect("spawn caffeine-awake owner thread");
        Mutex::new(tx)
    })
}

/// The only thread that ever calls SetThreadExecutionState.
fn owner(rx: Receiver<Cmd>) {
    let mut asserted = false;
    loop {
        match if asserted {
            rx.recv_timeout(REASSERT_INTERVAL)
        } else {
            // Block indefinitely; nothing to re-assert while released.
            rx.recv().map_err(|_| RecvTimeoutError::Disconnected)
        } {
            Ok(cmd) => {
                set(cmd.on);
                asserted = cmd.on;
                if let Some(ack) = cmd.ack {
                    let _ = ack.send(());
                }
            }
            Err(RecvTimeoutError::Timeout) => set(true),
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn set(on: bool) {
    unsafe {
        if on {
            let _ =
                SetThreadExecutionState(ES_CONTINUOUS | ES_DISPLAY_REQUIRED | ES_SYSTEM_REQUIRED);
        } else {
            let _ = SetThreadExecutionState(ES_CONTINUOUS);
        }
    }
}

/// Assert or release the keep-awake state. Blocks until the owner thread has
/// applied it (bounded), so toggling off is guaranteed to stick even though
/// the caller runs on a borrowed FRB worker thread.
pub fn apply_awake(on: bool) {
    let (ack_tx, ack_rx) = channel::<()>();
    let sent = sender()
        .lock()
        .map(|g| g.send(Cmd { on, ack: Some(ack_tx) }).is_ok())
        .unwrap_or(false);
    if sent {
        let _ = ack_rx.recv_timeout(Duration::from_secs(2));
    }
}
