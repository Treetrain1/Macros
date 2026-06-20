use std::collections::HashSet;
use std::sync::atomic::{AtomicI32, AtomicU32, AtomicUsize, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use enigo::{Enigo, Keyboard, Mouse, Settings};
use tracing::warn;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN,
    GetAsyncKeyState, RegisterHotKey, UnregisterHotKey, VIRTUAL_KEY,
    VK_BACK, VK_CAPITAL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_F1,
    VK_F10, VK_F11, VK_F12, VK_HOME, VK_INSERT, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN,
    VK_LEFT, VK_NEXT, VK_NUMLOCK, VK_NUMPAD0, VK_NUMPAD1, VK_NUMPAD2, VK_NUMPAD3,
    VK_NUMPAD4, VK_NUMPAD5, VK_NUMPAD6, VK_NUMPAD7, VK_NUMPAD8, VK_NUMPAD9,
    VK_PAUSE, VK_PRIOR, VK_RCONTROL, VK_RETURN, VK_RIGHT,
    VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SCROLL, VK_SNAPSHOT, VK_SPACE, VK_TAB, VK_UP,
    VK_VOLUME_DOWN, VK_VOLUME_MUTE, VK_VOLUME_UP,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetForegroundWindow, GetWindow, GetWindowThreadProcessId,
    IsIconic, IsWindowVisible, PostThreadMessageW, SetForegroundWindow, SetWindowsHookExW,
    GW_HWNDNEXT, KBDLLHOOKSTRUCT, MSLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL,
    WM_APP, WM_HOTKEY, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN,
    WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_RBUTTONUP,
    WM_SYSKEYDOWN, WM_XBUTTONDOWN, WM_XBUTTONUP,
};

use crate::hotkey_types::{
    HotkeyAction, HotkeyBinding, MOD_ALT as MALT, MOD_CTRL as MCTRL, MOD_META as MMETA,
    MOD_SHIFT as MSHIFT,
};
use crate::input::types::{Axis, Direction, MacroButton, MacroKey};
use crate::macros::backend::{CaptureDecision, CaptureEvent, CaptureTimestamp, InputBackend};

// ── Globals ───────────────────────────────────────────────────────────────────

static CALLBACK: OnceLock<Mutex<Box<dyn FnMut(CaptureEvent, CaptureTimestamp) -> CaptureDecision + Send + 'static>>> =
    OnceLock::new();
// VK codes whose key-down was suppressed; used to also suppress the matching key-up.
static SUPPRESSED_KEYS: OnceLock<Mutex<HashSet<u16>>> = OnceLock::new();
// Track last absolute cursor position to compute relative deltas.
static LAST_CURSOR_X: AtomicI32 = AtomicI32::new(i32::MIN);
static LAST_CURSOR_Y: AtomicI32 = AtomicI32::new(i32::MIN);
// Foreground window (HWND as isize) at the moment a hotkey fires.
static HOTKEY_FOREGROUND_HWND: AtomicUsize = AtomicUsize::new(0);
// VK code + time of the most recent SendInput-injected key-down; used to tell
// WM_HOTKEY apart from macro playback pressing its own hotkey combo.
static LAST_INJECTED_KEYDOWN: OnceLock<Mutex<Option<(u16, Instant)>>> = OnceLock::new();

// ── RegisterHotKey state ──────────────────────────────────────────────────────

// Windows thread ID of the hook message loop thread.
static HOOK_THREAD_ID: AtomicU32 = AtomicU32::new(0);
// Maps hotkey ID → HotkeyAction for currently registered hotkeys.
static REGISTERED_HOTKEYS: OnceLock<Mutex<Vec<(i32, HotkeyAction)>>> = OnceLock::new();
// Pending bindings written by signal_hotkey_update, read by the hook thread.
static PENDING_BINDINGS: OnceLock<Mutex<Option<Vec<HotkeyBinding>>>> = OnceLock::new();

use std::sync::Mutex;

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

/// Maps the hotkey name strings produced by `MacroKey::hotkey_name()` to Win32 VK codes.
fn hotkey_name_to_vk(name: &str) -> Option<VIRTUAL_KEY> {
    if let Some(rest) = name.strip_prefix("Key") {
        let c = rest.chars().next()?;
        if c.is_ascii_uppercase() {
            return Some(0x41 + (c as u16 - b'A' as u16));
        }
    }
    if let Some(rest) = name.strip_prefix("Num") {
        let c = rest.chars().next()?;
        if c.is_ascii_digit() {
            return Some(0x30 + (c as u16 - b'0' as u16));
        }
    }
    Some(match name {
        "Return" => VK_RETURN,
        "Backspace" => VK_BACK,
        "Tab" => VK_TAB,
        "Space" => VK_SPACE,
        "Escape" => VK_ESCAPE,
        "Delete" => VK_DELETE,
        "Insert" => VK_INSERT,
        "Home" => VK_HOME,
        "End" => VK_END,
        "PageUp" => VK_PRIOR,
        "PageDown" => VK_NEXT,
        "UpArrow" => VK_UP,
        "DownArrow" => VK_DOWN,
        "LeftArrow" => VK_LEFT,
        "RightArrow" => VK_RIGHT,
        "F1" => VK_F1,
        "F2" => 0x71,
        "F3" => 0x72,
        "F4" => 0x73,
        "F5" => 0x74,
        "F6" => 0x75,
        "F7" => 0x76,
        "F8" => 0x77,
        "F9" => 0x78,
        "F10" => VK_F10,
        "F11" => VK_F11,
        "F12" => VK_F12,
        "CapsLock" => VK_CAPITAL,
        "NumLock" => VK_NUMLOCK,
        "ScrollLock" => VK_SCROLL,
        "Pause" => VK_PAUSE,
        "PrintScreen" => VK_SNAPSHOT,
        "Minus" => 0xBD,
        "Equal" => 0xBB,
        "LeftBracket" => 0xDB,
        "RightBracket" => 0xDD,
        "BackSlash" => 0xDC,
        "SemiColon" => 0xBA,
        "Quote" => 0xDE,
        "BackQuote" => 0xC0,
        "Comma" => 0xBC,
        "Dot" => 0xBE,
        "Slash" => 0xBF,
        _ => return None,
    })
}

/// Converts the app's modifier bitmask (MOD_CTRL/SHIFT/ALT/META) to Win32 RegisterHotKey flags.
fn combo_mods_to_winapi(mods: u8) -> u32 {
    let mut r = 0u32;
    if mods & MCTRL != 0 {
        r |= MOD_CONTROL as u32;
    }
    if mods & MSHIFT != 0 {
        r |= MOD_SHIFT as u32;
    }
    if mods & MALT != 0 {
        r |= MOD_ALT as u32;
    }
    if mods & MMETA != 0 {
        r |= MOD_WIN as u32;
    }
    r
}

/// Converts Win32 RegisterHotKey modifier flags (as delivered in a WM_HOTKEY
/// message's lParam LOWORD) back to the app's MOD_CTRL/SHIFT/ALT/META bitmask.
fn winapi_mods_to_combo(mods: u32) -> u8 {
    let mut r = 0u8;
    if mods & MOD_CONTROL as u32 != 0 {
        r |= MCTRL;
    }
    if mods & MOD_SHIFT as u32 != 0 {
        r |= MSHIFT;
    }
    if mods & MOD_ALT as u32 != 0 {
        r |= MALT;
    }
    if mods & MOD_WIN as u32 != 0 {
        r |= MMETA;
    }
    r
}

/// (Un)registers hotkeys via the Win32 RegisterHotKey API. Must be called from the hook thread.
fn register_hotkeys_on_hook_thread(bindings: &[HotkeyBinding]) {
    let reg = REGISTERED_HOTKEYS.get_or_init(|| Mutex::new(vec![]));
    if let Ok(mut hotkeys) = reg.lock() {
        for (id, _) in &*hotkeys {
            unsafe { UnregisterHotKey(std::ptr::null_mut(), *id); }
        }
        hotkeys.clear();
        for (idx, binding) in bindings.iter().enumerate() {
            let id = (idx as i32) + 1;
            let Some(vk) = hotkey_name_to_vk(&binding.combo.key) else {
                warn!("No VK mapping for hotkey key {:?}", binding.combo.key);
                continue;
            };
            let mods = combo_mods_to_winapi(binding.combo.modifiers) | MOD_NOREPEAT as u32;
            if unsafe { RegisterHotKey(std::ptr::null_mut(), id, mods, vk as u32) } != 0 {
                hotkeys.push((id, binding.action.clone()));
            } else {
                warn!("RegisterHotKey failed for {:?}", binding.combo.key);
            }
        }
    }
}

/// Called from any thread when the hotkey binding table changes.
/// Stores the new bindings and wakes up the hook thread to re-register them.
pub(crate) fn signal_hotkey_update(bindings: Vec<HotkeyBinding>) {
    let pending = PENDING_BINDINGS.get_or_init(|| Mutex::new(None));
    if let Ok(mut g) = pending.lock() {
        *g = Some(bindings);
    }
    let tid = HOOK_THREAD_ID.load(Ordering::Relaxed);
    if tid != 0 {
        unsafe { PostThreadMessageW(tid, WM_APP, 0, 0); }
    }
    // If tid == 0, the hook thread hasn't started yet; it will pick up
    // PENDING_BINDINGS immediately after startup.
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
        SUPPRESSED_KEYS.get_or_init(|| Mutex::new(HashSet::new()));

        std::thread::Builder::new()
            .name("winapi-hook".into())
            .spawn(|| unsafe {
                use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
                use windows_sys::Win32::System::Threading::GetCurrentThreadId;
                use windows_sys::Win32::UI::WindowsAndMessaging::{
                    DispatchMessageW, GetMessageW, MSG, TranslateMessage,
                };

                crate::macros::priority::raise_current_thread_priority();
                HOOK_THREAD_ID.store(GetCurrentThreadId(), Ordering::Relaxed);

                let hinstance = GetModuleHandleW(std::ptr::null());
                let _kb_hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), hinstance, 0);
                let _ms_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), hinstance, 0);

                // Register any bindings that arrived before this thread started.
                if let Some(p) = PENDING_BINDINGS.get() {
                    if let Ok(mut g) = p.lock() {
                        if let Some(bindings) = g.take() {
                            register_hotkeys_on_hook_thread(&bindings);
                        }
                    }
                }

                let mut msg: MSG = std::mem::zeroed();
                while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) != 0 {
                    if msg.message == WM_HOTKEY {
                        let id = msg.wParam as i32;
                        // HIWORD(lParam) = triggering VK code. If macro playback just
                        // injected this same key, this WM_HOTKEY is the app reacting to
                        // its own output rather than a real user keypress — ignore it.
                        let vk = ((msg.lParam as usize >> 16) & 0xFFFF) as u16;
                        let was_injected = LAST_INJECTED_KEYDOWN.get_or_init(|| Mutex::new(None))
                            .lock()
                            .ok()
                            .and_then(|g| *g)
                            .map(|(last_vk, t)| last_vk == vk && t.elapsed() < Duration::from_millis(100))
                            .unwrap_or(false);
                        let hwnd = GetForegroundWindow();
                        HOTKEY_FOREGROUND_HWND.store(hwnd as usize, Ordering::Relaxed);
                        if !was_injected && !crate::recording::RECORDING_ACTIVE.load(Ordering::Relaxed) {
                            if let Some(reg) = REGISTERED_HOTKEYS.get() {
                                if let Ok(hotkeys) = reg.lock() {
                                    if let Some((_, action)) =
                                        hotkeys.iter().find(|(hid, _)| *hid == id)
                                    {
                                        if matches!(action, HotkeyAction::StartRecordingImmediate) {
                                            // LOWORD(lParam) = the MOD_* flags held when the
                                            // hotkey fired. RegisterHotKey fires once on
                                            // press only, so defer the actual start until
                                            // these keys (tracked via the regular keyboard
                                            // hook) are all released.
                                            let win_mods = (msg.lParam as usize & 0xFFFF) as u32;
                                            let app_mods = winapi_mods_to_combo(win_mods);
                                            if let Some(macro_key) = vk_to_macro_key(vk) {
                                                crate::recording::arm_pending_record_start(
                                                    app_mods, macro_key,
                                                );
                                            }
                                        } else {
                                            crate::recording::push_queue_signal(
                                                crate::recording::QueueSignal::Hotkey(action.clone()),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    } else if msg.message == WM_APP {
                        if let Some(p) = PENDING_BINDINGS.get() {
                            if let Ok(mut g) = p.lock() {
                                if let Some(bindings) = g.take() {
                                    register_hotkeys_on_hook_thread(&bindings);
                                }
                            }
                        }
                    } else {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            })
            .ok();
    });
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
            if w_param == WM_KEYDOWN as usize || w_param == WM_SYSKEYDOWN as usize {
                if let Ok(mut g) = LAST_INJECTED_KEYDOWN.get_or_init(|| Mutex::new(None)).lock() {
                    *g = Some((kb.vkCode as u16, Instant::now()));
                }
            }
            return unsafe { CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param) };
        }
        let pressed = w_param == WM_KEYDOWN as usize || w_param == WM_SYSKEYDOWN as usize;
        let vk = kb.vkCode as u16;
        if let Some(macro_key) = vk_to_macro_key(vk) {
            let suppress = if pressed {
                let decision = CALLBACK.get()
                    .and_then(|cb| cb.lock().ok())
                    .map(|mut cb| cb(CaptureEvent::KeyPress(macro_key), CaptureTimestamp::Now))
                    .unwrap_or(CaptureDecision::Passthrough);
                if matches!(decision, CaptureDecision::Suppress) {
                    if let Some(set) = SUPPRESSED_KEYS.get() {
                        if let Ok(mut s) = set.lock() { s.insert(vk); }
                    }
                    true
                } else {
                    false
                }
            } else {
                let was_suppressed = SUPPRESSED_KEYS.get()
                    .and_then(|s| s.lock().ok())
                    .map(|mut s| s.remove(&vk))
                    .unwrap_or(false);
                let cb_suppress = CALLBACK.get()
                    .and_then(|cb| cb.lock().ok())
                    .map(|mut cb| matches!(cb(CaptureEvent::KeyRelease(macro_key), CaptureTimestamp::Now), CaptureDecision::Suppress))
                    .unwrap_or(false);
                was_suppressed || cb_suppress
            };
            if suppress { return 1; }
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

        let last_x = LAST_CURSOR_X.load(Ordering::Relaxed);
        let last_y = LAST_CURSOR_Y.load(Ordering::Relaxed);
        let cur_x = ms.pt.x;
        let cur_y = ms.pt.y;
        LAST_CURSOR_X.store(cur_x, Ordering::Relaxed);
        LAST_CURSOR_Y.store(cur_y, Ordering::Relaxed);

        // 0x01 = LLMHF_INJECTED: skip SendInput events so macro playback
        // doesn't feed back into the recording system.
        if ms.flags & 0x01 != 0 {
            return unsafe { CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param) };
        }

        let suppress = if let Some(cb) = CALLBACK.get() {
            if let Ok(mut cb) = cb.lock() {
                let decision = match w_param as u32 {
                    WM_MOUSEMOVE => {
                        if last_x != i32::MIN {
                            let dx = cur_x - last_x;
                            let dy = cur_y - last_y;
                            if dx != 0 || dy != 0 {
                                cb(CaptureEvent::MouseMoveRel(dx, dy), CaptureTimestamp::Now)
                            } else {
                                CaptureDecision::Passthrough
                            }
                        } else {
                            CaptureDecision::Passthrough
                        }
                    }
                    WM_LBUTTONDOWN => cb(CaptureEvent::ButtonPress(MacroButton::Left), CaptureTimestamp::Now),
                    WM_LBUTTONUP => cb(CaptureEvent::ButtonRelease(MacroButton::Left), CaptureTimestamp::Now),
                    WM_RBUTTONDOWN => cb(CaptureEvent::ButtonPress(MacroButton::Right), CaptureTimestamp::Now),
                    WM_RBUTTONUP => cb(CaptureEvent::ButtonRelease(MacroButton::Right), CaptureTimestamp::Now),
                    WM_MBUTTONDOWN => cb(CaptureEvent::ButtonPress(MacroButton::Middle), CaptureTimestamp::Now),
                    WM_MBUTTONUP => cb(CaptureEvent::ButtonRelease(MacroButton::Middle), CaptureTimestamp::Now),
                    WM_XBUTTONDOWN => match (ms.mouseData >> 16) as u16 {
                        1 => cb(CaptureEvent::ButtonPress(MacroButton::Back), CaptureTimestamp::Now),
                        2 => cb(CaptureEvent::ButtonPress(MacroButton::Forward), CaptureTimestamp::Now),
                        _ => CaptureDecision::Passthrough,
                    },
                    WM_XBUTTONUP => match (ms.mouseData >> 16) as u16 {
                        1 => cb(CaptureEvent::ButtonRelease(MacroButton::Back), CaptureTimestamp::Now),
                        2 => cb(CaptureEvent::ButtonRelease(MacroButton::Forward), CaptureTimestamp::Now),
                        _ => CaptureDecision::Passthrough,
                    },
                    WM_MOUSEWHEEL => {
                        let delta = (ms.mouseData >> 16) as i16;
                        let ticks = delta as i32 / 120;
                        cb(CaptureEvent::Scroll(0, ticks), CaptureTimestamp::Now)
                    }
                    WM_MOUSEHWHEEL => {
                        let delta = (ms.mouseData >> 16) as i16;
                        let ticks = delta as i32 / 120;
                        cb(CaptureEvent::Scroll(ticks, 0), CaptureTimestamp::Now)
                    }
                    _ => CaptureDecision::Passthrough,
                };
                matches!(decision, CaptureDecision::Suppress)
            } else {
                false
            }
        } else {
            false
        };

        if suppress {
            return 1;
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param) }
}

fn vk_to_macro_key(vk: VIRTUAL_KEY) -> Option<MacroKey> {
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

/// Call before executing a hotkey-triggered macro. Restores focus to the window
/// that was active when the hotkey fired and releases any held modifier keys.
pub(crate) fn prepare_for_macro_execution() {
    let stored: HWND = HOTKEY_FOREGROUND_HWND.load(Ordering::Relaxed) as HWND;

    if !stored.is_null() {
        unsafe {
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(stored, &mut pid);

            if pid == std::process::id() {
                // The macro app was foreground — find the first visible,
                // non-minimised window owned by another process.
                let mut candidate = GetWindow(stored, GW_HWNDNEXT);
                while !candidate.is_null() {
                    let mut cpid: u32 = 0;
                    GetWindowThreadProcessId(candidate, &mut cpid);
                    if cpid != std::process::id()
                        && IsWindowVisible(candidate) != 0
                        && IsIconic(candidate) == 0
                    {
                        SetForegroundWindow(candidate);
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        break;
                    }
                    candidate = GetWindow(candidate, GW_HWNDNEXT);
                }
            } else {
                SetForegroundWindow(stored);
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    }

    // Release any physically-held modifier keys.
    if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
        for &vk in &[
            VK_LCONTROL, VK_RCONTROL,
            VK_LSHIFT, VK_RSHIFT,
            VK_LMENU, VK_RMENU,
            VK_LWIN, VK_RWIN,
        ] {
            if unsafe { GetAsyncKeyState(vk as i32) } < 0 {
                let _ = enigo.key(enigo::Key::Other(vk as u32), enigo::Direction::Release);
            }
        }
    }
}
