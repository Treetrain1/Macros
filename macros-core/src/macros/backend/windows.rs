use std::cell::RefCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use enigo::{Enigo, Keyboard, Mouse, Settings};
use tracing::{info, warn};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VIRTUAL_KEY,
    VK_BACK, VK_CAPITAL, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_F1,
    VK_F10, VK_F11, VK_F12, VK_HOME, VK_INSERT, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN,
    VK_LEFT, VK_MENU, VK_NEXT, VK_NUMLOCK, VK_NUMPAD0, VK_NUMPAD1, VK_NUMPAD2, VK_NUMPAD3,
    VK_NUMPAD4, VK_NUMPAD5, VK_NUMPAD6, VK_NUMPAD7, VK_NUMPAD8, VK_NUMPAD9,
    VK_PAUSE, VK_PRIOR, VK_RCONTROL, VK_RETURN, VK_RIGHT,
    VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SCROLL, VK_SHIFT, VK_SNAPSHOT, VK_SPACE, VK_TAB, VK_UP,
    VK_VOLUME_DOWN, VK_VOLUME_MUTE, VK_VOLUME_UP,
};
use windows_sys::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetForegroundWindow,
    IsIconic, IsWindow, IsWindowVisible, SetForegroundWindow, SetWindowsHookExW,
    EVENT_SYSTEM_DESKTOPSWITCH,
    KBDLLHOOKSTRUCT, MSLLHOOKSTRUCT,
    WH_KEYBOARD_LL, WH_MOUSE_LL, WINEVENT_OUTOFCONTEXT,
    WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN,
    WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_RBUTTONUP,
    WM_SYSKEYDOWN, WM_XBUTTONDOWN, WM_XBUTTONUP,
};

use crate::input::types::{Axis, Direction, MacroButton, MacroKey};
use crate::macros::backend::{CaptureDecision, CaptureEvent, CaptureTimestamp, InputBackend};

// ── Globals ───────────────────────────────────────────────────────────────────

static CALLBACK: OnceLock<Mutex<Box<dyn FnMut(CaptureEvent, CaptureTimestamp) -> CaptureDecision + Send + 'static>>> =
    OnceLock::new();

/// State the keyboard/mouse/desktop-switch hook procedures share. Plain
/// thread-local `RefCell`, not a `Mutex` or atomics: `WH_KEYBOARD_LL` and
/// `WH_MOUSE_LL` hook procedures, and a `WINEVENT_OUTOFCONTEXT` WinEvent
/// callback, are all delivered exclusively on the thread that installed
/// them — here, the single `winapi-hook` thread spawned in
/// `start_capture_thread` below, which installs all four and then pumps the
/// message queue that delivers them. There is no second thread to race
/// against, so synchronization primitives here would be guarding against
/// something that can't happen.
///
/// (`HOTKEY_FOREGROUND_HWND` further down is hook-derived state a genuinely
/// different thread does read — macro playback, restoring focus before a
/// run — which is why it stays an atomic.)
struct HookThreadState {
    /// VK codes currently physically held down. Indexed directly — a VK code
    /// is documented as 1..=254 — rather than hashed, so telling a real press
    /// from an auto-repeat is a plain array read. `KBDLLHOOKSTRUCT` carries no
    /// repeat flag of its own; that's only synthesized later, in the
    /// `WM_KEYDOWN` lParam a window procedure would receive, which a global
    /// low-level hook runs upstream of.
    key_held: [bool; 256],
    /// VK codes whose key-down was swallowed, so the matching key-up is
    /// swallowed too rather than reaching the focused window unpaired.
    key_suppressed: [bool; 256],
    /// Last absolute cursor position seen, to turn `WM_MOUSEMOVE`'s absolute
    /// coordinates into the relative deltas the rest of the app works in.
    /// `None` until the first move is seen, rather than a sentinel coordinate:
    /// a magic `i32::MIN` used to sit here and its unguarded subtraction was
    /// exactly the overflow that used to panic this thread on startup.
    last_cursor: Option<(i32, i32)>,
}

impl Default for HookThreadState {
    fn default() -> Self {
        Self { key_held: [false; 256], key_suppressed: [false; 256], last_cursor: None }
    }
}

thread_local! {
    static HOOK_STATE: RefCell<HookThreadState> = RefCell::new(HookThreadState::default());
}

// Foreground window (HWND as isize) at the moment a hotkey fires.
static HOTKEY_FOREGROUND_HWND: AtomicUsize = AtomicUsize::new(0);

/// Called from the shared capture callback when a hotkey combo matches, to
/// snapshot the window a hotkey-triggered macro should be typed back into.
/// Only meaningful at that instant — see `macro_target_window`.
pub fn note_hotkey_matched() {
    HOTKEY_FOREGROUND_HWND.store(unsafe { GetForegroundWindow() } as usize, Ordering::Relaxed);
}

// ── Key mapping ───────────────────────────────────────────────────────────────

/// Maps this app's `MacroKey` to an `enigo::Key`. `enigo::Key::Other(u32)` is
/// treated by enigo as a raw Windows VK code, so it's used here for keys with
/// no named `enigo::Key` equivalent (matching `MacroKey::Other`'s own
/// "raw VK code" semantics).
fn macro_key_to_enigo_key(key: &MacroKey) -> enigo::Key {
    use enigo::Key;
    match key {
        MacroKey::Return => Key::Return,
        MacroKey::Backspace => Key::Backspace,
        MacroKey::Tab => Key::Tab,
        MacroKey::Space => Key::Space,
        MacroKey::Escape => Key::Escape,
        MacroKey::Delete => Key::Delete,
        MacroKey::Insert => Key::Insert,
        MacroKey::Home => Key::Home,
        MacroKey::End => Key::End,
        MacroKey::PageUp => Key::PageUp,
        MacroKey::PageDown => Key::PageDown,
        MacroKey::UpArrow => Key::UpArrow,
        MacroKey::DownArrow => Key::DownArrow,
        MacroKey::LeftArrow => Key::LeftArrow,
        MacroKey::RightArrow => Key::RightArrow,
        MacroKey::Shift => Key::Shift,
        MacroKey::LShift => Key::LShift,
        MacroKey::RShift => Key::RShift,
        MacroKey::Control => Key::Control,
        MacroKey::LControl => Key::LControl,
        MacroKey::RControl => Key::RControl,
        MacroKey::Alt | MacroKey::Option => Key::Alt,
        MacroKey::AltGr => Key::Other(VK_RMENU as u32),
        MacroKey::Meta | MacroKey::LMenu => Key::Meta,
        MacroKey::CapsLock => Key::CapsLock,
        MacroKey::NumLock => Key::Numlock,
        MacroKey::ScrollLock => Key::Scroll,
        MacroKey::Pause => Key::Pause,
        MacroKey::PrintScr => Key::PrintScr,
        MacroKey::F1 => Key::F1,
        MacroKey::F2 => Key::F2,
        MacroKey::F3 => Key::F3,
        MacroKey::F4 => Key::F4,
        MacroKey::F5 => Key::F5,
        MacroKey::F6 => Key::F6,
        MacroKey::F7 => Key::F7,
        MacroKey::F8 => Key::F8,
        MacroKey::F9 => Key::F9,
        MacroKey::F10 => Key::F10,
        MacroKey::F11 => Key::F11,
        MacroKey::F12 => Key::F12,
        MacroKey::F13 => Key::F13,
        MacroKey::F14 => Key::F14,
        MacroKey::F15 => Key::F15,
        MacroKey::F16 => Key::F16,
        MacroKey::F17 => Key::F17,
        MacroKey::F18 => Key::F18,
        MacroKey::F19 => Key::F19,
        MacroKey::F20 => Key::F20,
        MacroKey::F21 => Key::F21,
        MacroKey::F22 => Key::F22,
        MacroKey::F23 => Key::F23,
        MacroKey::F24 => Key::F24,
        MacroKey::Numpad0 => Key::Numpad0,
        MacroKey::Numpad1 => Key::Numpad1,
        MacroKey::Numpad2 => Key::Numpad2,
        MacroKey::Numpad3 => Key::Numpad3,
        MacroKey::Numpad4 => Key::Numpad4,
        MacroKey::Numpad5 => Key::Numpad5,
        MacroKey::Numpad6 => Key::Numpad6,
        MacroKey::Numpad7 => Key::Numpad7,
        MacroKey::Numpad8 => Key::Numpad8,
        MacroKey::Numpad9 => Key::Numpad9,
        MacroKey::Add => Key::Add,
        MacroKey::Subtract => Key::Subtract,
        MacroKey::Multiply => Key::Multiply,
        MacroKey::Divide => Key::Divide,
        MacroKey::Decimal => Key::Decimal,
        MacroKey::VolumeDown => Key::VolumeDown,
        MacroKey::VolumeMute => Key::VolumeMute,
        MacroKey::VolumeUp => Key::VolumeUp,
        MacroKey::Select => Key::Other(0x29),
        MacroKey::Unicode(c) => Key::Unicode(*c),
        MacroKey::Other(n) => Key::Other(*n),
    }
}

fn to_enigo_dir(dir: Direction) -> enigo::Direction {
    match dir {
        Direction::Press => enigo::Direction::Press,
        Direction::Release => enigo::Direction::Release,
        Direction::Click => enigo::Direction::Click,
    }
}

// ── InputBackend impl ─────────────────────────────────────────────────────────

pub struct WinApiBackend {
    enigo: Enigo,
}

impl WinApiBackend {
    pub fn new() -> Result<Self, String> {
        let settings = Settings {
            // Match the previous SendInput-based implementation: relative
            // mouse moves are raw MOUSEEVENTF_MOVE deltas, subject to the
            // user's OS pointer-speed/acceleration settings, rather than
            // enigo's default of converting them to an absolute move.
            windows_subject_to_mouse_speed_and_acceleration_level: true,
            // The previous implementation never auto-released held keys on
            // drop; modifier cleanup is handled explicitly in
            // `prepare_for_macro_execution`.
            release_keys_when_dropped: false,
            ..Default::default()
        };
        let enigo = Enigo::new(&settings).map_err(|e| e.to_string())?;
        Ok(Self { enigo })
    }
}

impl InputBackend for WinApiBackend {
    fn key(&mut self, key: MacroKey, dir: Direction) -> Result<(), String> {
        let key = macro_key_to_enigo_key(&key);
        self.enigo.key(key, to_enigo_dir(dir)).map_err(|e| e.to_string())
    }

    fn raw_keycode(&mut self, keycode: u16, dir: Direction) -> Result<(), String> {
        self.enigo
            .key(enigo::Key::Other(keycode as u32), to_enigo_dir(dir))
            .map_err(|e| e.to_string())
    }

    fn button(&mut self, button: MacroButton, dir: Direction) -> Result<(), String> {
        use enigo::{Axis as EAxis, Button as EButton};
        let dir = to_enigo_dir(dir);
        match button {
            // enigo's vertical scroll sign convention is inverted relative to
            // the raw MOUSEEVENTF_WHEEL delta this used to send directly, so
            // negate to keep "ScrollUp" actually scrolling up.
            MacroButton::ScrollUp => self.enigo.scroll(-1, EAxis::Vertical),
            MacroButton::ScrollDown => self.enigo.scroll(1, EAxis::Vertical),
            MacroButton::ScrollLeft => self.enigo.scroll(-1, EAxis::Horizontal),
            MacroButton::ScrollRight => self.enigo.scroll(1, EAxis::Horizontal),
            MacroButton::Left => self.enigo.button(EButton::Left, dir),
            MacroButton::Right => self.enigo.button(EButton::Right, dir),
            MacroButton::Middle => self.enigo.button(EButton::Middle, dir),
            MacroButton::Back => self.enigo.button(EButton::Back, dir),
            MacroButton::Forward => self.enigo.button(EButton::Forward, dir),
            MacroButton::Other(_) => Ok(()),
        }
        .map_err(|e| e.to_string())
    }

    fn move_mouse_rel(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        self.enigo
            .move_mouse(dx, dy, enigo::Coordinate::Rel)
            .map_err(|e| e.to_string())
    }

    fn move_mouse_abs(&mut self, x: i32, y: i32) -> Result<(), String> {
        self.enigo
            .move_mouse(x, y, enigo::Coordinate::Abs)
            .map_err(|e| e.to_string())
    }

    fn scroll(&mut self, amount: i32, axis: Axis) -> Result<(), String> {
        // enigo negates the vertical length internally before sending
        // MOUSEEVENTF_WHEEL, so negate here to keep `amount`'s sign matching
        // the raw wheel-delta convention this trait used before (positive =
        // up). Horizontal isn't negated by enigo, so it passes through as-is.
        let (length, axis) = match axis {
            Axis::Vertical => (-amount, enigo::Axis::Vertical),
            Axis::Horizontal => (amount, enigo::Axis::Horizontal),
        };
        self.enigo.scroll(length, axis).map_err(|e| e.to_string())
    }

    fn text(&mut self, s: &str) -> Result<(), String> {
        self.enigo.text(s).map_err(|e| e.to_string())
    }

    fn cursor_pos(&self) -> Option<(i32, i32)> {
        self.enigo.location().ok()
    }
}

// ── Capture ───────────────────────────────────────────────────────────────────

pub(super) fn start_capture_thread(
    callback: Box<dyn FnMut(CaptureEvent, CaptureTimestamp) -> CaptureDecision + Send + 'static>,
) {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        CALLBACK
            .set(Mutex::new(callback))
            .unwrap_or_else(|_| warn!("Capture callback already set"));

        std::thread::Builder::new()
            .name("winapi-hook".into())
            .spawn(|| unsafe {
                use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
                use windows_sys::Win32::UI::WindowsAndMessaging::{
                    DispatchMessageW, GetMessageW, MSG, TranslateMessage,
                };

                crate::macros::priority::raise_current_thread_priority();

                let hinstance = GetModuleHandleW(std::ptr::null());
                let kb_hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), hinstance, 0);
                let ms_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), hinstance, 0);
                // Fires on any switch to/from a secure desktop (UAC prompt,
                // Ctrl+Alt+Del, lock screen) — the situations where the
                // keyboard hook misses key-ups and `HELD_MODS` would wedge.
                let ds_hook = SetWinEventHook(
                    EVENT_SYSTEM_DESKTOPSWITCH,
                    EVENT_SYSTEM_DESKTOPSWITCH,
                    std::ptr::null_mut(),
                    Some(desktop_switch_event_proc),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT,
                );

                // The process id matters here: CEF runs helper processes by
                // re-executing this same binary, and a helper that reached
                // this code would install a competing hook whose queue nobody
                // consumes. Exactly one of these lines should ever appear.
                if kb_hook.is_null() || ms_hook.is_null() || ds_hook.is_null() {
                    warn!(
                        pid = std::process::id(),
                        keyboard_hook = !kb_hook.is_null(),
                        mouse_hook = !ms_hook.is_null(),
                        desktop_switch_hook = !ds_hook.is_null(),
                        "Some input hooks failed to install; hotkeys and recording will be degraded"
                    );
                } else {
                    info!(pid = std::process::id(), "Input hooks installed");
                }
                // Surfaced in the UI the same way Linux reports a failed
                // evdev grab and macOS a refused event tap. Keyed on the two
                // hooks that actually carry input: without them there is no
                // recording and no hotkeys, which the user needs telling
                // about rather than finding out by pressing a dead combo.
                crate::recording::set_grab_failed(kb_hook.is_null() || ms_hook.is_null());

                // Nothing is dispatched here beyond keeping the queue pumped:
                // both low-level hooks and the WinEvent hook above are
                // delivered as callbacks from inside GetMessageW, and hotkeys
                // are matched in `keyboard_proc` rather than arriving as
                // messages.
                let mut msg: MSG = std::mem::zeroed();
                while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) != 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            })
            .ok();
    });
}

/// Windows silently removes a low-level hook whose procedure takes longer than
/// `LowLevelHooksTimeout` (HKCU\Control Panel\Desktop, 300 ms by default) to
/// return — no error, no notification, and no way to query it afterwards.
/// Hotkeys and recording just stop working until the app is restarted, which
/// is indistinguishable from a matching bug.
///
/// Every hook procedure runs the shared capture callback through here so that
/// approaching the budget is at least visible in the log. The threshold is a
/// third of the default, to leave warning before events start being dropped.
///
/// Nothing on the callback's path should get close: it is atomics, a
/// non-blocking `try_read` of the hotkey table, and a lock-free channel send,
/// plus one mutex that is drained only when a recording stops. If this ever
/// fires, that assumption has been broken and the callback needs moving onto a
/// dispatch thread the way the evdev backend already does it.
const HOOK_BUDGET_WARN: std::time::Duration = std::time::Duration::from_millis(100);

/// Runs the shared capture callback, timed against the hook budget above.
fn dispatch(event: CaptureEvent) -> CaptureDecision {
    let started = std::time::Instant::now();
    let decision = CALLBACK
        .get()
        .and_then(|cb| cb.lock().ok())
        .map(|mut cb| cb(event, CaptureTimestamp::Now))
        .unwrap_or(CaptureDecision::Passthrough);
    let elapsed = started.elapsed();
    if elapsed >= HOOK_BUDGET_WARN {
        warn!(
            elapsed_ms = elapsed.as_millis(),
            "Capture callback approached the low-level hook timeout; past \
             LowLevelHooksTimeout Windows unhooks us silently and input stops \
             being seen at all"
        );
    }
    decision
}

/// Repeat-tracking for `dispatch_from_focused_window`, separate from
/// `HOOK_STATE`: that state is thread-local to the `winapi-hook` thread, but
/// this path is fed from CEF's own UI thread (see its doc comment below), so
/// it needs its own bookkeeping to tell a fresh press from auto-repeat.
///
/// Timestamp-based rather than a held/released flag pair: CEF doesn't
/// reliably deliver a matching `KEYUP` through `on_pre_key_event` once a
/// key-down has been reported handled (returning suppress for a matched
/// hotkey does exactly that), so a flag set on press and cleared on release
/// can get stuck permanently "held" — every later press of that key would
/// then look like a repeat of a release that's never coming, and silently
/// stop firing. A short elapsed-time check can't get stuck: it only ever
/// looks at how long ago the last press was.
static FOCUSED_KEY_LAST_PRESS: Mutex<[Option<std::time::Instant>; 256]> = Mutex::new([None; 256]);

/// Auto-repeat delivers the next `RAWKEYDOWN` well under this apart (Windows'
/// fastest repeat-rate setting is ~33ms between repeats); a deliberate second
/// press of the same key is comfortably slower than that.
const FOCUSED_KEY_REPEAT_WINDOW: std::time::Duration = std::time::Duration::from_millis(60);

/// Feeds a key event from the app's own webview into the same hotkey
/// pipeline the global OS hook uses, for the one case that hook can't cover:
/// Chromium grabs raw keyboard input for its own focused window on Windows
/// (https://github.com/chromiumembedded/cef/issues/2609), which starves
/// `WH_KEYBOARD_LL` while this app's window has focus. The CEF client's
/// `on_pre_key_event` still sees every keystroke in that case, so it's wired
/// to call here instead. Returns whether to suppress the key.
pub(crate) fn dispatch_from_focused_window(vk: u16, pressed: bool) -> bool {
    if vk as usize >= 256 {
        return false;
    }
    if pressed {
        let now = std::time::Instant::now();
        let is_repeat = FOCUSED_KEY_LAST_PRESS
            .lock()
            .map(|mut last| {
                let prev = last[vk as usize].replace(now);
                prev.is_some_and(|p| now.duration_since(p) < FOCUSED_KEY_REPEAT_WINDOW)
            })
            .unwrap_or(false);
        if is_repeat {
            return false;
        }
    }
    let Some(macro_key) = vk_to_macro_key(vk) else {
        return false;
    };
    let event = if pressed {
        CaptureEvent::KeyPress(macro_key)
    } else {
        CaptureEvent::KeyRelease(macro_key)
    };
    matches!(dispatch(event), CaptureDecision::Suppress)
}

/// Any key held when the desktop switches (UAC prompt, Ctrl+Alt+Del, lock
/// screen) will have its key-up delivered to the secure desktop, where our
/// hook can't see it. Every tracker is cleared here because a missed key-up
/// wedges each of them: the shared modifier state would keep reporting a
/// phantom modifier (no combo matches again), a stale `key_suppressed` entry
/// would make the hook swallow that key system-wide forever, and a stale
/// `key_held` one would make its next real press look like an auto-repeat and
/// go unreported. Clearing is safe — the user isn't mid-combo across a
/// desktop switch, and anything genuinely still held on return re-registers
/// on its next press.
unsafe extern "system" fn desktop_switch_event_proc(
    _hook: HWINEVENTHOOK,
    event: u32,
    _hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    if event == EVENT_SYSTEM_DESKTOPSWITCH {
        crate::recording::reset_held_mods();
        HOOK_STATE.with_borrow_mut(|s| {
            s.key_held = [false; 256];
            s.key_suppressed = [false; 256];
        });
    }
}

unsafe extern "system" fn keyboard_proc(
    n_code: i32,
    w_param: usize,
    l_param: isize,
) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::HC_ACTION;
    if n_code == HC_ACTION as i32 {
        let kb = unsafe { &*(l_param as *const KBDLLHOOKSTRUCT) };
        // 0x10 = LLKHF_INJECTED: skip events injected by SendInput so macro
        // playback doesn't feed back into the recording system.
        if kb.flags & 0x10 != 0 {
            return unsafe { CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param) };
        }
        let pressed = w_param == WM_KEYDOWN as usize || w_param == WM_SYSKEYDOWN as usize;
        let vk = kb.vkCode as usize;
        if vk >= 256 {
            // Documented range is 1..=254, but this hook sees whatever any
            // process on the system injects — guard the array index rather
            // than trust an external contract, the way an unchecked
            // assumption in this same file already caused a panic once (see
            // `last_cursor` above).
            return unsafe { CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param) };
        }

        // Hotkey matching, StopRecording and recording capture all happen in
        // the shared callback (`recording::start_grab_thread`), against the
        // same `MacroKey` values Linux and macOS report. This hook's only
        // extra jobs are telling auto-repeat apart from a real press and
        // keeping key-up suppression paired with its key-down.
        let is_repeat = pressed && HOOK_STATE.with_borrow_mut(|s| {
            // Already held means this key-down is a repeat, not a fresh press.
            std::mem::replace(&mut s.key_held[vk], true)
        });
        if !pressed {
            HOOK_STATE.with_borrow_mut(|s| s.key_held[vk] = false);
        }
        if is_repeat {
            // Repeats of a suppressed key keep being swallowed — that's what
            // stops a held-down combo firing its action once per repeat. Every
            // other repeat still reaches the focused window (swallowing those
            // would break held-key repeat system-wide); it just isn't reported
            // to the capture callback a second time.
            return if HOOK_STATE.with_borrow(|s| s.key_suppressed[vk]) {
                1
            } else {
                unsafe { CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param) }
            };
        }

        if let Some(macro_key) = vk_to_macro_key(vk as u16) {
            if pressed {
                // TEMP DIAGNOSTIC: confirms whether this hook is invoked at
                // all while our own window has focus. Remove once resolved.
                let fg = unsafe { GetForegroundWindow() };
                let mut fg_pid: u32 = 0;
                unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(fg, &mut fg_pid) };
                info!(
                    vk,
                    ?macro_key,
                    fg_is_self = (fg_pid == std::process::id()),
                    "keyboard_proc saw key-down"
                );
            }
            let event = if pressed {
                CaptureEvent::KeyPress(macro_key)
            } else {
                CaptureEvent::KeyRelease(macro_key)
            };
            let cb_suppress = matches!(dispatch(event), CaptureDecision::Suppress);

            let suppress = HOOK_STATE.with_borrow_mut(|s| {
                if pressed {
                    s.key_suppressed[vk] = cb_suppress;
                    cb_suppress
                } else {
                    // A key-up whose key-down was swallowed is swallowed too,
                    // however the callback answers — otherwise the focused
                    // window sees a release for a press it was never given.
                    std::mem::replace(&mut s.key_suppressed[vk], false) || cb_suppress
                }
            });
            if suppress {
                return 1;
            }
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param) }
}

unsafe extern "system" fn mouse_proc(
    n_code: i32,
    w_param: usize,
    l_param: isize,
) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::HC_ACTION;
    if n_code == HC_ACTION as i32 {
        let ms = unsafe { &*(l_param as *const MSLLHOOKSTRUCT) };

        // Updated even for injected events below, so the next real move's
        // delta is measured from wherever playback left the cursor.
        let last = HOOK_STATE.with_borrow_mut(|s| s.last_cursor.replace((ms.pt.x, ms.pt.y)));

        // 0x01 = LLMHF_INJECTED: skip SendInput events so macro playback
        // doesn't feed back into the recording system.
        if ms.flags & 0x01 != 0 {
            return unsafe { CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param) };
        }

        // `mouseData`'s high word is the X-button index for WM_XBUTTON*, and
        // the wheel delta (a multiple of WHEEL_DELTA = 120) for the wheels.
        let high_word = (ms.mouseData >> 16) as i16;
        let capture_ev: Option<CaptureEvent> = match w_param as u32 {
            // `None` means no position has been seen yet, so this event only
            // establishes the origin — there's no delta to report.
            WM_MOUSEMOVE => last.and_then(|(lx, ly)| {
                let (dx, dy) = (ms.pt.x - lx, ms.pt.y - ly);
                (dx != 0 || dy != 0).then_some(CaptureEvent::MouseMoveRel(dx, dy))
            }),
            WM_LBUTTONDOWN => Some(CaptureEvent::ButtonPress(MacroButton::Left)),
            WM_LBUTTONUP => Some(CaptureEvent::ButtonRelease(MacroButton::Left)),
            WM_RBUTTONDOWN => Some(CaptureEvent::ButtonPress(MacroButton::Right)),
            WM_RBUTTONUP => Some(CaptureEvent::ButtonRelease(MacroButton::Right)),
            WM_MBUTTONDOWN => Some(CaptureEvent::ButtonPress(MacroButton::Middle)),
            WM_MBUTTONUP => Some(CaptureEvent::ButtonRelease(MacroButton::Middle)),
            WM_XBUTTONDOWN => xbutton(high_word).map(CaptureEvent::ButtonPress),
            WM_XBUTTONUP => xbutton(high_word).map(CaptureEvent::ButtonRelease),
            WM_MOUSEWHEEL => Some(CaptureEvent::Scroll(0, high_word as i32 / 120)),
            WM_MOUSEHWHEEL => Some(CaptureEvent::Scroll(high_word as i32 / 120, 0)),
            _ => None,
        };

        if let Some(ev) = capture_ev {
            if matches!(dispatch(ev), CaptureDecision::Suppress) {
                return 1;
            }
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param) }
}

/// The X-button a `WM_XBUTTON*` message refers to. Anything beyond the two
/// Windows defines is left alone rather than guessed at.
fn xbutton(index: i16) -> Option<MacroButton> {
    match index {
        1 => Some(MacroButton::Back),
        2 => Some(MacroButton::Forward),
        _ => None,
    }
}

pub(crate) fn vk_to_macro_key(vk: VIRTUAL_KEY) -> Option<MacroKey> {
    Some(match vk {
        VK_RETURN => MacroKey::Return,
        VK_BACK => MacroKey::Backspace,
        VK_TAB => MacroKey::Tab,
        VK_SPACE => MacroKey::Space,
        VK_ESCAPE => MacroKey::Escape,
        VK_DELETE => MacroKey::Delete,
        VK_INSERT => MacroKey::Insert,
        VK_HOME => MacroKey::Home,
        VK_END => MacroKey::End,
        VK_PRIOR => MacroKey::PageUp,
        VK_NEXT => MacroKey::PageDown,
        VK_UP => MacroKey::UpArrow,
        VK_DOWN => MacroKey::DownArrow,
        VK_LEFT => MacroKey::LeftArrow,
        VK_RIGHT => MacroKey::RightArrow,
        VK_LSHIFT => MacroKey::LShift,
        VK_RSHIFT => MacroKey::RShift,
        VK_LCONTROL => MacroKey::LControl,
        VK_RCONTROL => MacroKey::RControl,
        VK_LMENU => MacroKey::Alt,
        VK_RMENU => MacroKey::AltGr,
        VK_LWIN => MacroKey::Meta,
        VK_RWIN => MacroKey::Meta,
        // A low-level hook reports the side-specific VK for a physically
        // pressed modifier, so these generic ones are rare. They're mapped
        // anyway because the shared hotkey matcher derives held modifiers from
        // `MacroKey::modifier_bit`, and the `Other(vk)` fallback below has no
        // modifier bit — a generic modifier landing there would silently drop
        // out of every combo.
        VK_SHIFT => MacroKey::Shift,
        VK_CONTROL => MacroKey::Control,
        VK_MENU => MacroKey::Alt,
        VK_CAPITAL => MacroKey::CapsLock,
        VK_NUMLOCK => MacroKey::NumLock,
        VK_SCROLL => MacroKey::ScrollLock,
        VK_PAUSE => MacroKey::Pause,
        VK_SNAPSHOT => MacroKey::PrintScr,
        VK_F1 => MacroKey::F1,
        0x71 => MacroKey::F2,
        0x72 => MacroKey::F3,
        0x73 => MacroKey::F4,
        0x74 => MacroKey::F5,
        0x75 => MacroKey::F6,
        0x76 => MacroKey::F7,
        0x77 => MacroKey::F8,
        0x78 => MacroKey::F9,
        VK_F10 => MacroKey::F10,
        VK_F11 => MacroKey::F11,
        VK_F12 => MacroKey::F12,
        VK_NUMPAD0 => MacroKey::Numpad0,
        VK_NUMPAD1 => MacroKey::Numpad1,
        VK_NUMPAD2 => MacroKey::Numpad2,
        VK_NUMPAD3 => MacroKey::Numpad3,
        VK_NUMPAD4 => MacroKey::Numpad4,
        VK_NUMPAD5 => MacroKey::Numpad5,
        VK_NUMPAD6 => MacroKey::Numpad6,
        VK_NUMPAD7 => MacroKey::Numpad7,
        VK_NUMPAD8 => MacroKey::Numpad8,
        VK_NUMPAD9 => MacroKey::Numpad9,
        VK_VOLUME_DOWN => MacroKey::VolumeDown,
        VK_VOLUME_MUTE => MacroKey::VolumeMute,
        VK_VOLUME_UP => MacroKey::VolumeUp,
        code @ 0x41..=0x5A => MacroKey::Unicode((code as u8 - 0x41 + b'a') as char),
        code @ 0x30..=0x39 => MacroKey::Unicode((code as u8 - 0x30 + b'0') as char),
        _ => MacroKey::Other(vk as u32),
    })
}

// ── Pre-macro focus + modifier cleanup ───────────────────────────────────────

/// True for a window that can actually be brought to the foreground right
/// now. Our own windows count too — a hotkey pressed while Macros itself is
/// focused should still type back into whichever of our windows had it.
fn is_usable_target(hwnd: HWND) -> bool {
    if hwnd.is_null() {
        return false;
    }
    unsafe { IsWindow(hwnd) != 0 && IsWindowVisible(hwnd) != 0 && IsIconic(hwnd) == 0 }
}

/// The window a hotkey-triggered macro should type into: whatever was
/// foreground when the hotkey fired. `None` means leave focus alone — e.g.
/// that window has since closed or been minimized.
fn macro_target_window() -> Option<HWND> {
    let stored: HWND = HOTKEY_FOREGROUND_HWND.load(Ordering::Relaxed) as HWND;
    is_usable_target(stored).then_some(stored)
}

/// Call before executing a hotkey-triggered macro. Restores focus to the window
/// the macro should drive and releases any held modifier keys.
///
/// Takes the backend the macro is about to play through rather than building
/// an `Enigo` of its own: a throwaway one would be constructed with default
/// settings, not the tuned ones `WinApiBackend::new` picks, and these releases
/// are part of the same input stream the run is about to emit.
pub fn prepare_for_macro_execution(backend: &Arc<Mutex<dyn InputBackend>>) {
    if let Some(target) = macro_target_window() {
        unsafe {
            if GetForegroundWindow() != target {
                SetForegroundWindow(target);
                // Give the switch time to land before the first input goes
                // out, or the opening keystrokes are delivered to the old
                // foreground window.
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    }

    // Release any physically-held modifier keys, so a combo the user is still
    // holding from the hotkey doesn't modify everything the macro types.
    let Ok(mut backend) = backend.lock() else { return };
    for &vk in &[
        VK_LCONTROL, VK_RCONTROL,
        VK_LSHIFT, VK_RSHIFT,
        VK_LMENU, VK_RMENU,
        VK_LWIN, VK_RWIN,
    ] {
        if unsafe { GetAsyncKeyState(vk as i32) } < 0 {
            let _ = backend.raw_keycode(vk, Direction::Release);
        }
    }
}
