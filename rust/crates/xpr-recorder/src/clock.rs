//! The recorder's clock and how its events get processed.
//!
//! Live, the recorder follows the wall clock: settle times are real time, a
//! processing thread picks events up about 100 ms after they are queued, and
//! events are stamped with Super Shuckie's run timer.
//!
//! `replay_to_route` drives the recorder *offline* from a Super Shuckie
//! replay, many times faster than real time. There the clock is the replay's
//! time ([`set_offline_ms`]), queued events are processed when the driver
//! asks ([`crate::controller::Session::pump_events`]) instead of on a thread,
//! and events are stamped with the replay's run timer.
//!
//! An offline recording runs entirely on the thread that drives it, so the
//! offline state is per thread: an import in the app's background leaves a
//! live recording alone.

use std::cell::Cell;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

thread_local! {
    static OFFLINE: Cell<bool> = const { Cell::new(false) };
    static OFFLINE_MS: Cell<u64> = const { Cell::new(0) };
    /// the run timer events are stamped with offline
    static OFFLINE_TIMER_MS: Cell<Option<i64>> = const { Cell::new(None) };
}

static BASE: OnceLock<Instant> = OnceLock::new();

/// Switch this thread's recorder to offline mode (or back).
pub fn set_offline(offline: bool) {
    BASE.get_or_init(Instant::now);
    OFFLINE.with(|o| o.set(offline));
    if !offline {
        OFFLINE_MS.with(|m| m.set(0));
        OFFLINE_TIMER_MS.with(|t| t.set(None));
    }
}

pub fn is_offline() -> bool {
    OFFLINE.with(|o| o.get())
}

/// Offline: the replay's time now, in milliseconds since it started.
pub fn set_offline_ms(ms: u64) {
    OFFLINE_MS.with(|m| m.set(ms));
}

/// Offline: the run timer to stamp events with (`None`: no timer yet).
pub fn set_offline_timer_ms(ms: Option<i64>) {
    OFFLINE_TIMER_MS.with(|t| t.set(ms));
}

pub(crate) fn offline_timer_ms() -> Option<i64> {
    OFFLINE_TIMER_MS.with(|t| t.get())
}

/// `Instant::now()` for the recorder: the wall clock live, the replay's time
/// offline.
pub fn now() -> Instant {
    if is_offline() {
        let base = *BASE.get_or_init(Instant::now);
        base + Duration::from_millis(OFFLINE_MS.with(|m| m.get()))
    } else {
        Instant::now()
    }
}
