//! Thread-safe progress state shared between the sync/search worker thread
//! and the Tauri command that polls it every 500 ms.

use once_cell::sync::Lazy;
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::Mutex,
    time::Instant,
};

// ── Public snapshot type (sent to Angular) ────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ProgressSnapshot {
    pub active:      bool,
    pub phase:       String,
    pub done:        usize,
    pub total:       usize,
    pub current:     String,
    pub percent:     f32,
    pub eta_sec:     i64,
    pub errors:      usize,
    pub elapsed:     f32,
    pub file_types:  HashMap<String, FileTypeCount>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileTypeCount {
    pub done:  usize,
    pub total: usize,
}

// ── Internal mutable state ────────────────────────────────────────────────

struct ProgressState {
    phase:              String,
    done:               usize,
    total:              usize,
    current:            String,
    errors:             usize,
    file_types:         HashMap<String, FileTypeCount>,
    phase_start:        Option<Instant>,
    done_at_phase_start: usize,
}

impl Default for ProgressState {
    fn default() -> Self {
        Self {
            phase:               "idle".into(),
            done:                0,
            total:               0,
            current:             String::new(),
            errors:              0,
            file_types:          HashMap::new(),
            phase_start:         None,
            done_at_phase_start: 0,
        }
    }
}

static STATE: Lazy<Mutex<ProgressState>> =
    Lazy::new(|| Mutex::new(ProgressState::default()));

// ── Public API ────────────────────────────────────────────────────────────

pub fn set_progress(
    phase:   Option<&str>,
    done:    Option<usize>,
    total:   Option<usize>,
    current: Option<&str>,
    errors:  Option<usize>,
) {
    let mut s = STATE.lock().unwrap();

    if let Some(p) = phase {
        if p != s.phase {
            s.phase_start        = Some(Instant::now());
            s.done_at_phase_start = done.unwrap_or(s.done);
        }
        s.phase = p.to_string();
    }
    if let Some(v) = done    { s.done    = v; }
    if let Some(v) = total   { s.total   = v; }
    if let Some(v) = current { s.current = v.to_string(); }
    if let Some(v) = errors  { s.errors  = v; }
}

pub fn increment_errors() {
    STATE.lock().unwrap().errors += 1;
}

pub fn set_file_type_totals(counts: HashMap<String, usize>) {
    let mut s = STATE.lock().unwrap();
    s.file_types = counts
        .into_iter()
        .map(|(k, total)| (k, FileTypeCount { done: 0, total }))
        .collect();
}

pub fn increment_file_type(ext: &str) {
    let ext = ext.trim_start_matches('.').to_lowercase();
    let mut s = STATE.lock().unwrap();
    let entry = s.file_types.entry(ext).or_insert(FileTypeCount { done: 0, total: 1 });
    entry.done += 1;
}

pub fn get_progress() -> ProgressSnapshot {
    let s = STATE.lock().unwrap();

    let percent = if s.total > 0 {
        (s.done as f32 / s.total as f32 * 100.0 * 10.0).round() / 10.0
    } else {
        0.0
    };

    let (eta_sec, elapsed) = match s.phase_start {
        None => (-1_i64, 0.0_f32),
        Some(start) => {
            let elapsed  = start.elapsed().as_secs_f32();
            let items_done      = s.done.saturating_sub(s.done_at_phase_start);
            let items_remaining = s.total.saturating_sub(s.done);

            let eta = if elapsed > 2.0 && items_done > 0 && items_remaining > 0 {
                (items_remaining as f32 / (items_done as f32 / elapsed)).round() as i64
            } else {
                -1
            };
            (eta, (elapsed * 10.0).round() / 10.0)
        }
    };

    ProgressSnapshot {
        active:     s.phase != "idle",
        phase:      s.phase.clone(),
        done:       s.done,
        total:      s.total,
        current:    s.current.clone(),
        percent,
        eta_sec,
        errors:     s.errors,
        elapsed,
        file_types: s.file_types.clone(),
    }
}

pub fn reset() {
    let mut s = STATE.lock().unwrap();
    *s = ProgressState::default();
}
