//! C ABI surface for embedding the recording/playback engine directly into
//! a host process (e.g. the blockwork-gd Geode mod), replacing the loopback
//! TCP control connection `blockwork_core::ipc` provides for the standalone
//! desktop app. Every call here is synchronous and direct — no tokio
//! runtime, no `AppState`/`QueueSignal` bridging, since there's no GUI
//! event loop on the other side to hand off to.
//!
//! Every exported function is wrapped in `catch_unwind` so a Rust panic can
//! never unwind across the FFI boundary into the host's C++ stack.

#[cfg(windows)]
mod wine_bridge;

use blockwork_core::config;
use blockwork_core::macros::backend::InputBackend;
use blockwork_core::macros::run_registry;
use blockwork_core::macros::runner::{self, VariableStore};
use blockwork_core::macros::{Instruction, Macro, SPEED_MULTIPLIER_RANGE};
use blockwork_core::recording;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

static EMULATOR: OnceLock<Arc<Mutex<dyn InputBackend>>> = OnceLock::new();
#[cfg(windows)]
static WINE_BRIDGE: OnceLock<wine_bridge::WineBridge> = OnceLock::new();

/// A registered C callback that every `tracing::info!`/`warn!`/etc. call
/// site in this crate (and in `blockwork-core`, when linked in) gets routed
/// through — without this, `tracing` events have no subscriber and are
/// silently dropped, which is exactly what was happening before this was
/// added (confirmed empirically: zero Wine-bridge diagnostic lines ever
/// showed up despite the calls being present in the code).
static LOG_CALLBACK: AtomicUsize = AtomicUsize::new(0);

struct FfiLogWriter;

impl std::io::Write for FfiLogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let ptr = LOG_CALLBACK.load(Ordering::Relaxed);
        if ptr != 0 {
            if let Ok(s) = std::str::from_utf8(buf) {
                let line = s.trim_end_matches('\n');
                if !line.is_empty() {
                    if let Ok(cstr) = std::ffi::CString::new(line) {
                        let f: extern "C" fn(*const c_char) = unsafe { std::mem::transmute(ptr) };
                        f(cstr.as_ptr());
                    }
                }
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Registers the callback every `tracing` event gets formatted and routed
/// through — call once, before anything else, from the host's main thread
/// (e.g. right at the top of `$on_mod(Loaded)`, before `macros_init`).
/// `callback` receives one already-formatted, NUL-terminated line per
/// event; a null callback is accepted but makes this a no-op (events are
/// still silently dropped, same as before this existed).
#[no_mangle]
pub extern "C" fn macros_set_log_callback(callback: Option<extern "C" fn(*const c_char)>) {
    let ptr = callback.map(|f| f as usize).unwrap_or(0);
    LOG_CALLBACK.store(ptr, Ordering::Relaxed);

    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_writer(|| FfiLogWriter)
            .with_ansi(false)
            .without_time()
            .with_target(false)
            .try_init();
    });
}

fn catch(f: impl FnOnce() -> i32) -> i32 {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or(-99)
}

/// Same as `catch`, but for the string-returning functions below — a panic
/// becomes null instead of a bogus pointer.
fn catch_ptr(f: impl FnOnce() -> *mut c_char) -> *mut c_char {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or(std::ptr::null_mut())
}

fn owned_cstring(s: String) -> *mut c_char {
    CString::new(s).map(CString::into_raw).unwrap_or(std::ptr::null_mut())
}

/// # Safety
/// `ptr` must be null or point to a valid, NUL-terminated UTF-8 C string
/// that outlives this call.
unsafe fn cstr_to_owned(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(ptr) }.to_str().ok().map(str::to_owned)
}

/// Must be called once, from the host's main thread, before any other
/// `macros_*` function — on macOS the Accessibility prompt
/// (`request_accessibility`) requires the calling thread to be the main
/// thread. `config_dir_override_utf8` may be null/empty to use the OS
/// config directory as-is (the correct default on native Windows and
/// native macOS, where the embedder and any companion editor share one
/// filesystem); pass a path to redirect macro/settings storage elsewhere
/// (e.g. a Wine `Z:`-mapped path to the real desktop app's config dir when
/// running under Proton). `linux_bridge_resource_path_utf8` is the
/// Windows-side path (as `CCFileUtils` resolves it) to the bundled
/// `linux-input.so` resource — only used if this turns out to be running
/// under Wine on a Linux host (Proton); ignored (may be null) otherwise.
///
/// Returns 0 on success, -1 if no input backend could be created for this
/// platform.
#[no_mangle]
pub extern "C" fn macros_init(config_dir_override_utf8: *const c_char, linux_bridge_resource_path_utf8: *const c_char) -> i32 {
    let override_dir = unsafe { cstr_to_owned(config_dir_override_utf8) };
    let linux_bridge_resource_path = unsafe { cstr_to_owned(linux_bridge_resource_path_utf8) };
    catch(move || {
        if let Some(dir) = override_dir.filter(|d| !d.is_empty()) {
            config::set_config_dir_override(PathBuf::from(dir));
        }

        #[cfg(target_os = "macos")]
        {
            if !blockwork_core::macros::backend::macos::request_accessibility() {
                recording::set_grab_failed(true);
            }
        }

        // Deliberately not calling `recording::update_hotkey_table()`: the
        // physical-hotkey combo system stays permanently inert (every
        // `push_queue_signal` call site in `recording` is gated behind a
        // non-empty hotkey table), so control here is 100% driven by these
        // explicit FFI calls, never by incidental keys pressed in-game.

        #[cfg(windows)]
        if wine_bridge::detect_wine_linux() {
            let Some(path) = linux_bridge_resource_path.filter(|p| !p.is_empty()) else {
                tracing::warn!("blockwork-ffi: running under Wine but no linux-input resource path given; falling back to the native (non-functional under Wine) input path");
                return native_init();
            };
            return match wine_bridge::setup_and_launch(&path) {
                Some(bridge) => {
                    wine_bridge::spawn_heartbeat_thread(bridge.region);
                    wine_bridge::spawn_capture_forwarder(bridge.region);
                    // No local EMULATOR under Wine: macros_run_macro/
                    // macros_stop_loop route through WINE_BRIDGE instead
                    // (see their bodies) — the whole timed run, including
                    // emission, happens natively on the Linux side now, not
                    // here (see wire::WireControlCommand's docs for why).
                    let _ = WINE_BRIDGE.set(bridge);
                    0
                }
                None => {
                    tracing::warn!("blockwork-ffi: Wine bridge setup failed; falling back to the native (non-functional under Wine) input path");
                    native_init()
                }
            };
        }

        native_init()
    })
}

/// The non-Wine path: native OS hook capture + native emission backend.
/// Always correct on real Windows/macOS; also the (known non-functional)
/// fallback under Wine if the bridge can't be set up.
fn native_init() -> i32 {
    recording::start_grab_thread();
    match runner::make_backend() {
        Some(backend) => {
            let _ = EMULATOR.set(backend);
            0
        }
        None => -1,
    }
}

/// Starts recording into the currently-selected macro's timing baseline.
/// Mirrors the desktop app's `StartRecordingImmediate` hotkey handler.
#[no_mangle]
pub extern "C" fn macros_start_recording() -> i32 {
    catch(|| {
        recording::reset_timing();
        recording::RECORDING_ACTIVE.store(true, Ordering::Relaxed);
        0
    })
}

/// Stops recording and, if anything was captured, appends it to the
/// currently-selected macro's recording target and saves it to disk.
/// Returns the number of instructions captured and saved (0 means the
/// capture hook produced nothing at all — distinct from the negative error
/// codes below, which mean recording state couldn't even be resolved/saved).
#[no_mangle]
pub extern "C" fn macros_stop_recording() -> i32 {
    catch(|| {
        recording::RECORDING_ACTIVE.store(false, Ordering::Relaxed);

        let instructions: Vec<_> = match recording::get_recording_queue().lock() {
            Ok(mut queue) => queue.drain(..).collect(),
            Err(_) => return -1,
        };
        if instructions.is_empty() {
            return 0;
        }
        let count = instructions.len() as i32;

        let Some(id) = config::get_selected_macro_id() else { return -2 };
        let Some(mut mac) = config::get_macro_by_id(&id) else { return -2 };
        mac.recording_target_mut().instructions.extend(instructions);
        if let Err(e) = mac.save() {
            tracing::warn!("blockwork-ffi: failed to save recorded macro: {e}");
            return -3;
        }
        config::set_selected_macro_id(Some(&mac.id));
        count
    })
}

/// Runs a macro on a background thread — the explicit `id_utf8` if given
/// (nullable), otherwise the currently-selected macro, matching the
/// desktop app's `RunMacro`/`RunSpecificMacro` hotkey semantics. Loop mode
/// is read live from the shared settings file on every call, same as the
/// desktop app. Returns immediately; the run continues after this call
/// returns and is interruptible via `macros_stop_loop`.
///
/// `elapsed_overshoot_ms` (>= 0, milliseconds): how much real time had
/// already passed, before this call, since playback was actually supposed
/// to start — pass 0 if the caller has no such notion. `blockwork-gd`'s
/// attempt-start trigger only fires once per game frame, so it always
/// overshoots its own grace-period target by that frame's `dt`; passing
/// that overshoot here backdates the run's first `Wait` deadline to the
/// *intended* start instant instead of whenever this call happens to run,
/// via `Macro::run_with_offset`.
#[no_mangle]
pub extern "C" fn macros_run_macro(id_utf8: *const c_char, elapsed_overshoot_ms: f64) -> i32 {
    let requested_id = unsafe { cstr_to_owned(id_utf8) };
    catch(move || {
        let mac = requested_id
            .and_then(|id| config::get_macro_by_id(&id))
            .or_else(|| config::get_selected_macro_id().and_then(|id| config::get_macro_by_id(&id)));
        let Some(mac) = mac else { return -1 };

        // Under Wine, the entire timed run (loading the macro, pacing
        // Waits, emitting input) happens natively on the Linux side
        // instead — see wire::WireControlCommand's docs for why. Only the
        // resolved id crosses the bridge, never the macro data or any
        // per-event InputBackend calls.
        #[cfg(windows)]
        if let Some(bridge) = WINE_BRIDGE.get() {
            return match wine_bridge::send_run_macro(bridge.region, &mac.id, elapsed_overshoot_ms) {
                Ok(()) => 0,
                Err(e) => {
                    tracing::warn!("blockwork-ffi: failed to forward run_macro to Linux bridge: {e}");
                    -3
                }
            };
        }

        let Some(emulator) = EMULATOR.get().cloned() else { return -2 };

        let variables: VariableStore =
            Arc::new(Mutex::new(mac.variables.iter().map(|v| (v.name.clone(), v.value.clone())).collect()));
        let settings = config::load_settings();
        let speed_multiplier = mac.speed_multiplier * settings.global_speed_multiplier.unwrap_or(1.0);
        let loop_mode = settings.loop_mode_enabled.unwrap_or(false);
        let initial_offset = std::time::Duration::from_secs_f64(elapsed_overshoot_ms.max(0.0) / 1000.0);

        let flag = run_registry::begin_run();
        let spawned = std::thread::Builder::new().name("macros-run".to_string()).spawn(move || {
            let mut offset = initial_offset;
            loop {
                mac.clone().run_with_offset(Arc::clone(&emulator), Some(Arc::clone(&flag)), speed_multiplier, Arc::clone(&variables), offset);
                // Only the very first iteration corresponds to the real
                // attempt-start trigger; a loop-mode repeat restarting
                // immediately after has nothing to backdate against.
                offset = std::time::Duration::ZERO;
                let keep_looping = loop_mode && flag.lock().map(|g| *g).unwrap_or(false);
                if !keep_looping {
                    break;
                }
            }
            run_registry::end_run(&flag);
            if let Ok(values) = variables.lock() {
                let mut mac = mac.clone();
                mac.sync_variables_from(&values);
                if let Err(e) = mac.save() {
                    tracing::warn!("blockwork-ffi: failed to persist variable values: {e}");
                }
            }
        });

        match spawned {
            Ok(_) => 0,
            Err(_) => -3,
        }
    })
}

/// Stops every in-flight run (single or looping) started via
/// `macros_run_macro`.
#[no_mangle]
pub extern "C" fn macros_stop_loop() -> i32 {
    catch(|| {
        #[cfg(windows)]
        if let Some(bridge) = WINE_BRIDGE.get() {
            return match wine_bridge::send_stop_loop(bridge.region) {
                Ok(()) => 0,
                Err(e) => {
                    tracing::warn!("blockwork-ffi: failed to forward stop_loop to Linux bridge: {e}");
                    -1
                }
            };
        }

        run_registry::stop_all();
        0
    })
}

/// Sets the global playback speed multiplier — applied on top of each
/// macro's own per-macro `speed_multiplier` (see `Macro::speed_multiplier`)
/// — mirroring the desktop app's `set_global_speed_multiplier` command.
/// Clamped to `SPEED_MULTIPLIER_RANGE` (0.1x-10x). `macros_run_macro`
/// already reads this fresh from the shared settings file on every call,
/// so this takes effect starting with the next run; it does not speed up
/// or slow down a run already in flight. Always returns 0.
#[no_mangle]
pub extern "C" fn macros_set_speed_multiplier(multiplier: f64) -> i32 {
    catch(move || {
        let clamped = multiplier.clamp(*SPEED_MULTIPLIER_RANGE.start(), *SPEED_MULTIPLIER_RANGE.end());
        config::update_settings(|s| s.global_speed_multiplier = Some(clamped));
        0
    })
}

/// Creates a new macro (name defaults to `"New Macro"` if `name_utf8` is
/// null/empty), saves it, and marks it the currently-selected macro —
/// mirroring the desktop app's "+" button (the `new_macro` command).
/// Returns 0 on success, -1 if the new macro couldn't be saved.
#[no_mangle]
pub extern "C" fn macros_create_macro(name_utf8: *const c_char) -> i32 {
    let name = unsafe { cstr_to_owned(name_utf8) }
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "New Macro".to_string());
    catch(move || {
        let mac = Macro::new(name, String::new(), Vec::new());
        let id = mac.id.clone();
        if mac.add().is_err() {
            return -1;
        }
        config::set_selected_macro_id(Some(&id));
        0
    })
}

/// Returns a heap-allocated, NUL-terminated UTF-8 JSON array of every saved
/// macro, as `[{"id": "...", "name": "..."}, ...]`, in the same order the
/// desktop app's macro list uses (case-insensitive name, then id). The mod
/// has no delimited-text parser of its own but already carries Geode's
/// bundled `matjson` transitively, so real JSON is handed back rather than
/// inventing a bespoke format. Never null (an empty library serializes to
/// `"[]"`); caller must free the result with `macros_free_string`.
#[no_mangle]
pub extern "C" fn macros_list_macros() -> *mut c_char {
    #[derive(serde::Serialize)]
    struct MacroSummary {
        id: String,
        name: String,
    }

    catch_ptr(|| {
        let summaries: Vec<MacroSummary> = config::get_macros_from_config()
            .into_iter()
            .map(|mac| MacroSummary { id: mac.id, name: mac.name })
            .collect();
        owned_cstring(serde_json::to_string(&summaries).unwrap_or_else(|_| "[]".to_string()))
    })
}

/// Returns a heap-allocated, NUL-terminated UTF-8 string with the id of the
/// currently-selected macro, or null if none is selected. Caller must free a
/// non-null result with `macros_free_string`.
#[no_mangle]
pub extern "C" fn macros_get_selected_macro_id() -> *mut c_char {
    catch_ptr(|| match config::get_selected_macro_id() {
        Some(id) => owned_cstring(id),
        None => std::ptr::null_mut(),
    })
}

/// Marks the macro with the given id as the currently-selected one — the
/// target `macros_run_macro(NULL)`, `macros_stop_recording`, and
/// `macros_clear_recording_target_instructions` all resolve against. Returns
/// 0 on success, -1 if no macro with that id exists.
#[no_mangle]
pub extern "C" fn macros_select_macro(id_utf8: *const c_char) -> i32 {
    let Some(id) = (unsafe { cstr_to_owned(id_utf8) }) else { return -1 };
    catch(move || {
        if config::get_macro_by_id(&id).is_none() {
            return -1;
        }
        config::set_selected_macro_id(Some(&id));
        0
    })
}

/// Renames the currently-selected macro and saves it. `name_utf8`'s
/// surrounding whitespace is trimmed; a null/empty/whitespace-only name is
/// rejected rather than silently saving a blank title (unlike the desktop
/// app's own rename field, which has no such guard — but the mod's popup
/// has no live validation feedback, so this is the one place that has to
/// catch it). Returns 0 on success, or a negative error code: -1 no macro
/// currently selected, -2 selected macro couldn't be loaded, -3 the
/// trimmed name was empty, -4 couldn't be saved back to disk.
#[no_mangle]
pub extern "C" fn macros_rename_macro(name_utf8: *const c_char) -> i32 {
    let name = unsafe { cstr_to_owned(name_utf8) }.unwrap_or_default();
    catch(move || {
        let Some(id) = config::get_selected_macro_id() else { return -1 };
        let Some(mut mac) = config::get_macro_by_id(&id) else { return -2 };

        let trimmed = name.trim();
        if trimmed.is_empty() {
            return -3;
        }
        mac.name = trimmed.to_string();

        if let Err(e) = mac.save() {
            tracing::warn!("blockwork-ffi: failed to save renamed macro: {e}");
            return -4;
        }
        0
    })
}

/// Permanently deletes the currently-selected macro's file from disk and
/// clears the selection (mirroring the desktop app's "Remove Macro"
/// button, minus its two-step confirm timer — the mod's own UI puts up its
/// own confirmation before ever calling this). Returns 0 on success, or a
/// negative error code: -1 no macro currently selected, -2 selected macro
/// couldn't be loaded (already gone?), -3 couldn't be removed from disk.
#[no_mangle]
pub extern "C" fn macros_delete_macro() -> i32 {
    catch(|| {
        let Some(id) = config::get_selected_macro_id() else { return -1 };
        let Some(mac) = config::get_macro_by_id(&id) else { return -2 };

        if let Err(e) = mac.remove() {
            tracing::warn!("blockwork-ffi: failed to delete macro: {e}");
            return -3;
        }
        config::set_selected_macro_id(None);
        0
    })
}

/// Clears every recorded instruction from the currently-selected macro's
/// recording-target strand only (see `Macro::recording_target_mut`) —
/// unlike the desktop app's "Clear Instructions" button, which wipes every
/// strand in the macro, this leaves every other strand untouched, and keeps
/// the target strand's own `WhenRan`/`BlockHeader` if it has one (so it
/// stays a valid entry point / block body for the next recording instead of
/// disappearing). Returns the number of instructions removed (0 if the
/// strand had none beyond its header), or a negative error code: -1 no
/// macro currently selected, -2 selected macro couldn't be loaded, -3
/// couldn't be saved back to disk.
#[no_mangle]
pub extern "C" fn macros_clear_recording_target_instructions() -> i32 {
    catch(|| {
        let Some(id) = config::get_selected_macro_id() else { return -1 };
        let Some(mut mac) = config::get_macro_by_id(&id) else { return -2 };

        let strand = mac.recording_target_mut();
        let keep = if strand.instructions.first().map_or(false, Instruction::is_header) { 1 } else { 0 };
        let removed = (strand.instructions.len() - keep) as i32;
        if removed == 0 {
            return 0;
        }
        strand.instructions.truncate(keep);

        if let Err(e) = mac.save() {
            tracing::warn!("blockwork-ffi: failed to save after clearing recording-target instructions: {e}");
            return -3;
        }
        removed
    })
}

/// Frees a string previously returned by `macros_list_macros` or
/// `macros_get_selected_macro_id`. Safe to call with null (no-op).
#[no_mangle]
pub extern "C" fn macros_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}

/// True if input capture/emission couldn't be set up — on macOS this means
/// Accessibility access hasn't been granted (to the host process); the
/// caller should surface a UI prompt to check System Settings.
#[no_mangle]
pub extern "C" fn macros_grab_failed() -> bool {
    recording::grab_failed()
}

/// Trivial ABI/link smoke-test symbol.
#[no_mangle]
pub extern "C" fn macros_ping() -> i32 {
    0
}
