// Tauri command handlers for the search pipeline.
//
// Two models sit behind `core::sidecar::is_ready()`, and they become ready at
// different times. The bundled mobilenet loads at sidecar startup,
// independent of login. The DINO embedding model is shipped encrypted and
// only loads once `services::auth` has pushed the licence key Supabase
// returns at token-validate — so readiness needs the health check to have
// passed, the embed model to be up, AND an active session.
//
// There is still no per-search key handoff: the key goes to the sidecar once,
// when auth obtains it, not on the search path.
//
// Progress is streamed to Angular via four Tauri events, each carrying the
// ApiResponse envelope:
//   search_progress  — snapshot while running
//   search_partial   — ranked/verified/rejected rows as they become known
//                       (SEARCH-LATENCY-PLAN.md Phase 4); the client merges
//                       these by path rather than replacing its result set
//   search_complete  — final, authoritative result set
//   search_error     — fatal error that stopped the search
// These are broadcast events tied to the single in-flight search rather than
// one particular invoke() call, so unlike other commands they carry no
// `requestId`.

use crate::{
    core::sidecar,
    models::response::{ApiResponse, ResponseSearchComplete, ResponseSearchPartial, ResponseSearchProgress, ResponseSidecarStatus, SearchProgressSnapshot},
    services::search,
};
use std::{path::PathBuf, sync::Mutex, time::Duration};
use tauri::Emitter;
use tokio::time;

// ── Search state — prevents concurrent searches ───────────────────────────

struct SearchState {
    running: bool,
}

static SEARCH_STATE: std::sync::OnceLock<Mutex<SearchState>> = std::sync::OnceLock::new();

fn search_state() -> &'static Mutex<SearchState> {
    SEARCH_STATE.get_or_init(|| Mutex::new(SearchState { running: false }))
}

// ── Command ───────────────────────────────────────────────────────────────

// Polled by the frontend to show/hide "Model is loading…" and gate the
// Search button, since the sidecar now starts at app launch rather than
// being loaded lazily on the first search. Follows the same
// emit-`<command>_response` convention as every other command here.
#[tauri::command]
pub fn sidecar_status(app: tauri::AppHandle, request_id: Option<String>) {
    let result = ApiResponse::ok(ResponseSidecarStatus {
        healthy: sidecar::is_healthy(),
        ready:   sidecar::is_ready(),
        // Broken out so the UI can tell "sidecar still booting" apart from
        // "waiting on the licensed model", which are minutes apart on a
        // cold start and have completely different causes when stuck.
        embed_ready: sidecar::is_embed_ready(),
    })
    .with_request_id(request_id);
    let _ = app.emit("sidecar_status_response", result);
}

fn emit_search_error(app: &tauri::AppHandle, message: impl Into<String>) {
    let payload: ApiResponse<()> = ApiResponse::err(409, message);
    let _ = app.emit("search_error", payload);
}

// `scope_paths` empty or omitted → search every watched folder in the Library.
// Otherwise the search is restricted to the provided folders.
#[tauri::command]
pub async fn search_start(
    app:         tauri::AppHandle,
    image_path:  String,
    scope_paths: Option<Vec<String>>,
    top_k:       usize,
) {
    // Guard: reject concurrent searches.
    {
        let mut state = search_state().lock().unwrap();
        if state.running {
            emit_search_error(&app, "A search is already in progress.");
            return;
        }
        state.running = true;
    }

    if !sidecar::is_ready() {
        emit_search_error(&app, "Still getting ready — please wait a moment and try again.");
        search_state().lock().unwrap().running = false;
        return;
    }

    // Query basename for the searching snapshot, captured before image_path is
    // moved into the worker closure below.
    let query_name = std::path::Path::new(&image_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    let app_clone = app.clone();
    // The initial "Searching" snapshot below (after spawn_blocking) also
    // needs `query_name`, so the worker closure gets its own clone rather
    // than moving the original in.
    let query_name_for_worker = query_name.clone();

    // Spawn the heavy work on a blocking thread so Tokio stays responsive.
    tokio::task::spawn_blocking(move || {
        let image = PathBuf::from(&image_path);
        let scope: Vec<PathBuf> = scope_paths
            .unwrap_or_default()
            .into_iter()
            .map(PathBuf::from)
            .collect();

        // Real progress during the verify phase — SIFT/RANSAC over a couple
        // hundred candidates is tens of seconds even parallelized
        // server-side, and a single frozen "Searching" snapshot for that
        // whole wait reads as hung. `search::execute` calls this between
        // verify chunks; `query_name` is the same basename the initial
        // snapshot below used, so the label doesn't jump when this takes over.
        // `mirrored` marks the second, flipped-query pass that only runs when
        // the first proved nothing (see `search::execute`). It gets its own
        // label because the bar restarting at zero would otherwise look like
        // the search had glitched and started over.
        //
        // Also emits `search_partial` (SEARCH-LATENCY-PLAN.md Phase 4)
        // whenever this tick carries updated rows — the very first call,
        // with the full "unchecked" ranking, and then once per verify chunk
        // as candidates are proven or rejected — so the grid can render
        // before `search_complete` ever fires.
        let on_verify_progress = |progress: search::VerifyProgress| {
            let percent = if progress.total > 0 {
                (progress.done as f32 / progress.total as f32 * 100.0).min(100.0)
            } else {
                0.0
            };
            let phase = if progress.mirrored { "Checking mirrored orientation" } else { "Verifying matches" }.to_string();
            let payload = ApiResponse::ok(ResponseSearchProgress {
                progress: SearchProgressSnapshot {
                    active:  true,
                    phase:   phase.clone(),
                    done:    progress.done,
                    total:   progress.total,
                    current: query_name_for_worker.clone(),
                    percent,
                    eta_sec: -1,
                    errors:  0,
                },
            });
            let _ = app_clone.emit("search_progress", payload);

            if !progress.updated.is_empty() {
                let partial = ApiResponse::ok(ResponseSearchPartial {
                    results: progress.updated.to_vec(),
                    done:    progress.done,
                    total:   progress.total,
                    phase,
                });
                let _ = app_clone.emit("search_partial", partial);
            }
        };

        match search::execute(&image, &scope, top_k, on_verify_progress) {
            Ok((results, failed_files)) => {
                let payload = ApiResponse::ok(ResponseSearchComplete { done: true, results, failed_files });
                let _ = app_clone.emit("search_complete", payload);
            }
            Err(e) => {
                let payload: ApiResponse<()> = e.to_response();
                let _ = app_clone.emit("search_error", payload);
            }
        }

        search_state().lock().unwrap().running = false;
    });

    // Move the UI from "Starting…" into an active searching state. This first
    // snapshot covers query description + stage-1 ranking, which really is
    // sub-second — `on_verify_progress` above takes over with real numbers
    // once the (much slower) SIFT/RANSAC verify phase starts.
    //
    // Crucially we do NOT read the global sync/index progress here: a background
    // folder reconcile writes to that same state, and surfacing it would make the
    // search screen show phantom "Indexing 4523/10000" progress that has nothing
    // to do with the search the user just ran.
    let initial = ApiResponse::ok(ResponseSearchProgress {
        progress: SearchProgressSnapshot {
            active:  true,
            phase:   "Searching".to_string(),
            done:    0,
            total:   0,
            current: query_name,
            percent: 0.0,
            eta_sec: -1,
            errors:  0,
        },
    });
    let _ = app.emit("search_progress", initial);

    // Wait for the blocking search to finish; it emits search_complete/_error.
    loop {
        time::sleep(Duration::from_millis(150)).await;
        if !search_state().lock().unwrap().running {
            break;
        }
    }
}
