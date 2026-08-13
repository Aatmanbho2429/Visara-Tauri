//! A cooperative "hold off, the user is searching" gate.
//!
//! Background maintenance work (the colour-tag backfill in `services::tags`)
//! runs on the same cores the sidecar needs for a search. Measured on a 2054-file
//! library: a search issued while that backfill was running took 4640 ms inside
//! the sidecar's PyTorch forward pass, against 27 ms for the identical work on
//! the identical image moments earlier — a ~174x stall purely from CPU
//! contention. A search is interactive and a backfill is not, so the backfill
//! yields.
//!
//! Deliberately cooperative rather than a lock: the background job decides
//! *where* it is safe to pause (between batches, never mid-write), and search
//! never blocks waiting to acquire anything. `Condvar` rather than a polled
//! flag so a paused job resumes the instant the last search finishes.

use once_cell::sync::Lazy;
use std::sync::{Condvar, Mutex};

/// (searches currently running, "that count changed" signal).  A count, not a
/// bool: searches are serialised at the command layer today, but a second
/// caller must not be able to clear the gate a first one still needs.
static STATE: Lazy<(Mutex<usize>, Condvar)> = Lazy::new(|| (Mutex::new(0), Condvar::new()));

/// Held for the lifetime of one search; clears its slot on drop, including on
/// the `?` early-returns scattered through `services::search::execute`.
pub struct SearchGuard(());

impl Drop for SearchGuard {
    fn drop(&mut self) {
        let (lock, cvar) = &*STATE;
        let mut active = lock.lock().unwrap_or_else(|e| e.into_inner());
        *active = active.saturating_sub(1);
        if *active == 0 {
            cvar.notify_all(); // wake every paused background job at once
        }
    }
}

/// Mark a search as in flight. Keep the returned guard alive for its duration.
#[must_use = "the gate is released as soon as the guard drops"]
pub fn begin() -> SearchGuard {
    let (lock, _) = &*STATE;
    let mut active = lock.lock().unwrap_or_else(|e| e.into_inner());
    *active += 1;
    SearchGuard(())
}

/// Block until no search is running. Call from background work at a point where
/// stopping is safe — between batches, not mid-transaction.
pub fn wait_while_active() {
    let (lock, cvar) = &*STATE;
    let mut active = lock.lock().unwrap_or_else(|e| e.into_inner());
    while *active > 0 {
        active = cvar.wait(active).unwrap_or_else(|e| e.into_inner());
    }
}

/// Whether a search is in flight right now. For logging and for loops that want
/// to notice a pause without committing to blocking on it.
pub fn is_active() -> bool {
    let (lock, _) = &*STATE;
    *lock.lock().unwrap_or_else(|e| e.into_inner()) > 0
}
