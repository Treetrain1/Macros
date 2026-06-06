use std::collections::HashSet;
use std::io;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Mutex, OnceLock};

use evdev::{AbsoluteAxisType, AttributeSet, EventType, InputEvent, Key, PropType, RelativeAxisType, uinput::VirtualDevice, uinput::VirtualDeviceBuilder};
use tracing::warn;

use crate::input::types::{Axis, Direction, MacroButton, MacroKey};
use crate::macros::backend::{CaptureDecision, CaptureEvent, InputBackend};

use super::evdev_mapping::{
    char_to_evdev, evdev_button_from_code, evdev_key_to_macro_key, macro_button_to_evdev,
    macro_key_to_evdev,
};

static VIRTUAL_DEVICE: OnceLock<Mutex<VirtualDevice>> = OnceLock::new();
static CURSOR_X: AtomicI32 = AtomicI32::new(0);
static CURSOR_Y: AtomicI32 = AtomicI32::new(0);

fn syn_event() -> InputEvent {
    InputEvent::new(EventType::SYNCHRONIZATION, 0, 0)
}

fn get_or_init_virtual_device() -> Result<&'static Mutex<VirtualDevice>, io::Error> {
    if let Some(vd) = VIRTUAL_DEVICE.get() {
        return Ok(vd);
    }
    let vd = build_virtual_device()?;
    // Ignore error if another thread initialized it first.
    let _ = VIRTUAL_DEVICE.set(Mutex::new(vd));
    VIRTUAL_DEVICE.get().ok_or_else(|| io::Error::other("virtual device init race"))
}

fn build_virtual_device() -> io::Result<VirtualDevice> {
    let mut keys = AttributeSet::<Key>::new();
    for k in [
        Key::KEY_A, Key::KEY_B, Key::KEY_C, Key::KEY_D, Key::KEY_E,
        Key::KEY_F, Key::KEY_G, Key::KEY_H, Key::KEY_I, Key::KEY_J,
        Key::KEY_K, Key::KEY_L, Key::KEY_M, Key::KEY_N, Key::KEY_O,
        Key::KEY_P, Key::KEY_Q, Key::KEY_R, Key::KEY_S, Key::KEY_T,
        Key::KEY_U, Key::KEY_V, Key::KEY_W, Key::KEY_X, Key::KEY_Y,
        Key::KEY_Z,
        Key::KEY_1, Key::KEY_2, Key::KEY_3, Key::KEY_4, Key::KEY_5,
        Key::KEY_6, Key::KEY_7, Key::KEY_8, Key::KEY_9, Key::KEY_0,
        Key::KEY_MINUS, Key::KEY_EQUAL, Key::KEY_LEFTBRACE, Key::KEY_RIGHTBRACE,
        Key::KEY_BACKSLASH, Key::KEY_SEMICOLON, Key::KEY_APOSTROPHE, Key::KEY_GRAVE,
        Key::KEY_COMMA, Key::KEY_DOT, Key::KEY_SLASH,
        Key::KEY_ENTER, Key::KEY_BACKSPACE, Key::KEY_TAB, Key::KEY_SPACE,
        Key::KEY_ESC, Key::KEY_DELETE, Key::KEY_INSERT,
        Key::KEY_HOME, Key::KEY_END, Key::KEY_PAGEUP, Key::KEY_PAGEDOWN,
        Key::KEY_UP, Key::KEY_DOWN, Key::KEY_LEFT, Key::KEY_RIGHT,
        Key::KEY_LEFTSHIFT, Key::KEY_RIGHTSHIFT,
        Key::KEY_LEFTCTRL, Key::KEY_RIGHTCTRL,
        Key::KEY_LEFTALT, Key::KEY_RIGHTALT,
        Key::KEY_LEFTMETA, Key::KEY_RIGHTMETA,
        Key::KEY_CAPSLOCK, Key::KEY_NUMLOCK, Key::KEY_SCROLLLOCK,
        Key::KEY_PAUSE, Key::KEY_SYSRQ,
        Key::KEY_F1, Key::KEY_F2, Key::KEY_F3, Key::KEY_F4,
        Key::KEY_F5, Key::KEY_F6, Key::KEY_F7, Key::KEY_F8,
        Key::KEY_F9, Key::KEY_F10, Key::KEY_F11, Key::KEY_F12,
        Key::KEY_F13, Key::KEY_F14, Key::KEY_F15, Key::KEY_F16,
        Key::KEY_F17, Key::KEY_F18, Key::KEY_F19, Key::KEY_F20,
        Key::KEY_F21, Key::KEY_F22, Key::KEY_F23, Key::KEY_F24,
        Key::KEY_KP0, Key::KEY_KP1, Key::KEY_KP2, Key::KEY_KP3,
        Key::KEY_KP4, Key::KEY_KP5, Key::KEY_KP6, Key::KEY_KP7,
        Key::KEY_KP8, Key::KEY_KP9,
        Key::KEY_KPPLUS, Key::KEY_KPMINUS, Key::KEY_KPASTERISK,
        Key::KEY_KPSLASH, Key::KEY_KPDOT, Key::KEY_KPENTER,
        Key::KEY_VOLUMEDOWN, Key::KEY_MUTE, Key::KEY_VOLUMEUP,
        Key::KEY_SELECT,
        Key::BTN_LEFT, Key::BTN_RIGHT, Key::BTN_MIDDLE,
        Key::BTN_BACK, Key::BTN_FORWARD,
    ] {
        keys.insert(k);
    }

    let mut rel_axes = AttributeSet::<RelativeAxisType>::new();
    for ax in [
        RelativeAxisType::REL_X,
        RelativeAxisType::REL_Y,
        RelativeAxisType::REL_WHEEL,
        RelativeAxisType::REL_HWHEEL,
    ] {
        rel_axes.insert(ax);
    }

    VirtualDeviceBuilder::new()?
        .name("macros-input")
        .with_keys(&keys)?
        .with_relative_axes(&rel_axes)?
        .build()
}

// ── Emission ──────────────────────────────────────────────────────────────────

fn emit_key(key: Key, pressed: bool) -> Result<(), String> {
    let vd = get_or_init_virtual_device().map_err(|e| e.to_string())?;
    let mut vd = vd.lock().unwrap();
    vd.emit(&[
        InputEvent::new(EventType::KEY, key.0, if pressed { 1 } else { 0 }),
        syn_event(),
    ])
    .map_err(|e| e.to_string())
}

fn emit_key_click(key: Key, needs_shift: bool) -> Result<(), String> {
    let vd = get_or_init_virtual_device().map_err(|e| e.to_string())?;
    let mut vd = vd.lock().unwrap();
    let mut events: Vec<InputEvent> = Vec::with_capacity(6);
    if needs_shift {
        events.push(InputEvent::new(EventType::KEY, Key::KEY_LEFTSHIFT.0, 1));
    }
    events.push(InputEvent::new(EventType::KEY, key.0, 1));
    events.push(InputEvent::new(EventType::KEY, key.0, 0));
    if needs_shift {
        events.push(InputEvent::new(EventType::KEY, Key::KEY_LEFTSHIFT.0, 0));
    }
    events.push(syn_event());
    vd.emit(&events).map_err(|e| e.to_string())
}

pub struct EvdevBackend;

impl EvdevBackend {
    pub fn new() -> Result<Self, io::Error> {
        get_or_init_virtual_device()?;
        Ok(Self)
    }
}

impl InputBackend for EvdevBackend {
    fn key(&mut self, key: MacroKey, dir: Direction) -> Result<(), String> {
        let (evdev_key, needs_shift) =
            macro_key_to_evdev(&key).ok_or_else(|| format!("no evdev mapping for {:?}", key))?;
        match dir {
            Direction::Press => {
                if needs_shift {
                    emit_key(Key::KEY_LEFTSHIFT, true)?;
                }
                emit_key(evdev_key, true)
            }
            Direction::Release => {
                let r = emit_key(evdev_key, false);
                if needs_shift {
                    emit_key(Key::KEY_LEFTSHIFT, false)?;
                }
                r
            }
            Direction::Click => emit_key_click(evdev_key, needs_shift),
        }
    }

    fn raw_keycode(&mut self, keycode: u16, dir: Direction) -> Result<(), String> {
        let key = Key(keycode);
        match dir {
            Direction::Press => emit_key(key, true),
            Direction::Release => emit_key(key, false),
            Direction::Click => emit_key_click(key, false),
        }
    }

    fn button(&mut self, button: MacroButton, dir: Direction) -> Result<(), String> {
        // Scroll variants become REL_WHEEL / REL_HWHEEL
        let (rel_axis, amount): (Option<RelativeAxisType>, i32) = match button {
            MacroButton::ScrollUp => (Some(RelativeAxisType::REL_WHEEL), 1),
            MacroButton::ScrollDown => (Some(RelativeAxisType::REL_WHEEL), -1),
            MacroButton::ScrollLeft => (Some(RelativeAxisType::REL_HWHEEL), -1),
            MacroButton::ScrollRight => (Some(RelativeAxisType::REL_HWHEEL), 1),
            _ => (None, 0),
        };
        if let Some(axis) = rel_axis {
            let vd = get_or_init_virtual_device().map_err(|e| e.to_string())?;
            let mut vd = vd.lock().unwrap();
            return vd
                .emit(&[InputEvent::new(EventType::RELATIVE, axis.0, amount), syn_event()])
                .map_err(|e| e.to_string());
        }

        let evdev_btn = macro_button_to_evdev(&button)
            .ok_or_else(|| format!("no evdev mapping for {:?}", button))?;
        match dir {
            Direction::Press => emit_key(evdev_btn, true),
            Direction::Release => emit_key(evdev_btn, false),
            Direction::Click => emit_key_click(evdev_btn, false),
        }
    }

    fn move_mouse_rel(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        CURSOR_X.fetch_add(dx, Ordering::Relaxed);
        CURSOR_Y.fetch_add(dy, Ordering::Relaxed);
        let vd = get_or_init_virtual_device().map_err(|e| e.to_string())?;
        let mut vd = vd.lock().unwrap();
        vd.emit(&[
            InputEvent::new(EventType::RELATIVE, RelativeAxisType::REL_X.0, dx),
            InputEvent::new(EventType::RELATIVE, RelativeAxisType::REL_Y.0, dy),
            syn_event(),
        ])
        .map_err(|e| e.to_string())
    }

    fn move_mouse_abs(&mut self, x: i32, y: i32) -> Result<(), String> {
        // Compute delta from tracked position and emit as relative.
        let cur_x = CURSOR_X.load(Ordering::Relaxed);
        let cur_y = CURSOR_Y.load(Ordering::Relaxed);
        let dx = x - cur_x;
        let dy = y - cur_y;
        self.move_mouse_rel(dx, dy)
    }

    fn scroll(&mut self, amount: i32, axis: Axis) -> Result<(), String> {
        let (rel, val) = match axis {
            Axis::Vertical => (RelativeAxisType::REL_WHEEL, amount),
            Axis::Horizontal => (RelativeAxisType::REL_HWHEEL, amount),
        };
        let vd = get_or_init_virtual_device().map_err(|e| e.to_string())?;
        let mut vd = vd.lock().unwrap();
        vd.emit(&[InputEvent::new(EventType::RELATIVE, rel.0, val), syn_event()])
            .map_err(|e| e.to_string())
    }

    fn text(&mut self, s: &str) -> Result<(), String> {
        for c in s.chars() {
            match char_to_evdev(c) {
                Some((key, needs_shift)) => emit_key_click(key, needs_shift)?,
                None => warn!("no evdev mapping for char {:?}, skipping", c),
            }
        }
        Ok(())
    }

    fn cursor_pos(&self) -> Option<(i32, i32)> {
        Some((
            CURSOR_X.load(Ordering::Relaxed),
            CURSOR_Y.load(Ordering::Relaxed),
        ))
    }
}

// ── Capture ───────────────────────────────────────────────────────────────────

enum DeviceMsg {
    KeyPress { key: Key, raw: InputEvent },
    KeyRelease { key: Key, raw: InputEvent },
    ButtonPress { key: Key, raw: InputEvent },
    ButtonRelease { key: Key, raw: InputEvent },
    MouseMove { dx: i32, dy: i32, raw_x: Option<InputEvent>, raw_y: Option<InputEvent> },
    Scroll { v: i32, h: i32, raw_v: Option<InputEvent>, raw_h: Option<InputEvent> },
    OtherBatch(Vec<InputEvent>),
}

pub(super) fn start_capture_thread(
    mut callback: Box<dyn FnMut(CaptureEvent) -> CaptureDecision + Send + 'static>,
) {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        let vd = match get_or_init_virtual_device() {
            Ok(vd) => vd,
            Err(e) => {
                warn!("Cannot start evdev capture (no virtual device): {}", e);
                return;
            }
        };

        let (tx, rx) = std::sync::mpsc::sync_channel::<DeviceMsg>(256);

        // Enumerate and grab physical input devices.
        let grabbed: Vec<_> = evdev::enumerate()
            .filter_map(|(path, mut device)| {
                let name = device.name().unwrap_or("").to_owned();
                if name == "macros-input" {
                    return None;
                }
                // Skip touchpads and absolute pointer devices (tablets, trackpads).
                // Touchpads report ABS_X and/or carry the BUTTONPAD property.
                // External mice use only relative axes and will have neither.
                let is_buttonpad = device.properties().contains(PropType::BUTTONPAD);
                let has_abs = device
                    .supported_absolute_axes()
                    .map(|s| s.contains(AbsoluteAxisType::ABS_X))
                    .unwrap_or(false);
                if is_buttonpad || has_abs {
                    return None;
                }
                let has_keys = device
                    .supported_keys()
                    .map(|s| s.contains(Key::KEY_A) || s.contains(Key::BTN_LEFT))
                    .unwrap_or(false);
                let has_rel = device
                    .supported_relative_axes()
                    .map(|s| s.contains(RelativeAxisType::REL_X))
                    .unwrap_or(false);
                if !has_keys && !has_rel {
                    return None;
                }
                match device.grab() {
                    Ok(()) => Some(device),
                    Err(e) => {
                        warn!("Failed to grab {:?} ({}): {}", path, name, e);
                        None
                    }
                }
            })
            .collect();

        if grabbed.is_empty() {
            warn!("No input devices could be grabbed; global hotkeys and recording unavailable.");
        }

        // Spawn one reader thread per grabbed device.
        for mut device in grabbed {
            let tx = tx.clone();
            std::thread::Builder::new()
                .name("evdev-reader".into())
                .spawn(move || {
                    let mut pending_dx = 0i32;
                    let mut pending_dy = 0i32;
                    let mut pending_wv = 0i32;
                    let mut pending_wh = 0i32;
                    let mut raw_x: Option<InputEvent> = None;
                    let mut raw_y: Option<InputEvent> = None;
                    let mut raw_wv: Option<InputEvent> = None;
                    let mut raw_wh: Option<InputEvent> = None;
                    let mut other_batch: Vec<InputEvent> = Vec::new();

                    loop {
                        let events = match device.fetch_events() {
                            Ok(e) => e,
                            Err(e) => {
                                warn!("evdev read error: {}", e);
                                break;
                            }
                        };

                        for event in events {
                            match event.event_type() {
                                EventType::KEY => {
                                    if !other_batch.is_empty() {
                                        let _ = tx.send(DeviceMsg::OtherBatch(
                                            std::mem::take(&mut other_batch),
                                        ));
                                    }
                                    let key = Key(event.code());
                                    let value = event.value();
                                    if value == 2 {
                                        // Autorepeat: pass through silently
                                        let _ = tx.send(DeviceMsg::OtherBatch(vec![event]));
                                        continue;
                                    }
                                    let is_btn = key.0 >= 0x100; // BTN_MISC = 0x100
                                    let pressed = value == 1;
                                    let msg = if is_btn {
                                        if pressed {
                                            DeviceMsg::ButtonPress { key, raw: event }
                                        } else {
                                            DeviceMsg::ButtonRelease { key, raw: event }
                                        }
                                    } else if pressed {
                                        DeviceMsg::KeyPress { key, raw: event }
                                    } else {
                                        DeviceMsg::KeyRelease { key, raw: event }
                                    };
                                    let _ = tx.send(msg);
                                }
                                EventType::RELATIVE => {
                                    match RelativeAxisType(event.code()) {
                                        RelativeAxisType::REL_X => {
                                            pending_dx += event.value();
                                            raw_x = Some(event);
                                        }
                                        RelativeAxisType::REL_Y => {
                                            pending_dy += event.value();
                                            raw_y = Some(event);
                                        }
                                        RelativeAxisType::REL_WHEEL => {
                                            pending_wv += event.value();
                                            raw_wv = Some(event);
                                        }
                                        RelativeAxisType::REL_HWHEEL => {
                                            pending_wh += event.value();
                                            raw_wh = Some(event);
                                        }
                                        _ => other_batch.push(event),
                                    }
                                }
                                EventType::SYNCHRONIZATION => {
                                    if !other_batch.is_empty() {
                                        let _ = tx.send(DeviceMsg::OtherBatch(
                                            std::mem::take(&mut other_batch),
                                        ));
                                    }
                                    if pending_dx != 0 || pending_dy != 0 {
                                        let _ = tx.send(DeviceMsg::MouseMove {
                                            dx: std::mem::replace(&mut pending_dx, 0),
                                            dy: std::mem::replace(&mut pending_dy, 0),
                                            raw_x: raw_x.take(),
                                            raw_y: raw_y.take(),
                                        });
                                    }
                                    if pending_wv != 0 || pending_wh != 0 {
                                        let _ = tx.send(DeviceMsg::Scroll {
                                            v: std::mem::replace(&mut pending_wv, 0),
                                            h: std::mem::replace(&mut pending_wh, 0),
                                            raw_v: raw_wv.take(),
                                            raw_h: raw_wh.take(),
                                        });
                                    }
                                }
                                _ => other_batch.push(event),
                            }
                        }
                    }
                })
                .ok();
        }

        // Dispatch thread: single-threaded callback + re-emission.
        std::thread::Builder::new()
            .name("evdev-dispatch".into())
            .spawn(move || {
                let mut suppressed_keys: HashSet<u16> = HashSet::new();

                while let Ok(msg) = rx.recv() {
                    match msg {
                        DeviceMsg::KeyPress { key, raw } => {
                            let macro_key = match evdev_key_to_macro_key(key) {
                                Some(k) => k,
                                None => {
                                    reemit(vd, &[raw]);
                                    continue;
                                }
                            };
                            match callback(CaptureEvent::KeyPress(macro_key)) {
                                CaptureDecision::Passthrough => {
                                    suppressed_keys.remove(&key.0);
                                    reemit(vd, &[raw]);
                                }
                                CaptureDecision::Suppress => {
                                    suppressed_keys.insert(key.0);
                                }
                            }
                        }
                        DeviceMsg::KeyRelease { key, raw } => {
                            let macro_key = match evdev_key_to_macro_key(key) {
                                Some(k) => k,
                                None => {
                                    reemit(vd, &[raw]);
                                    continue;
                                }
                            };
                            let was_suppressed = suppressed_keys.remove(&key.0);
                            match callback(CaptureEvent::KeyRelease(macro_key)) {
                                CaptureDecision::Passthrough if !was_suppressed => {
                                    reemit(vd, &[raw]);
                                }
                                _ => {}
                            }
                        }
                        DeviceMsg::ButtonPress { key, raw } => {
                            let btn = match evdev_button_from_code(key.0) {
                                Some(b) => b,
                                None => {
                                    reemit(vd, &[raw]);
                                    continue;
                                }
                            };
                            match callback(CaptureEvent::ButtonPress(btn)) {
                                CaptureDecision::Passthrough => reemit(vd, &[raw]),
                                CaptureDecision::Suppress => {}
                            }
                        }
                        DeviceMsg::ButtonRelease { key, raw } => {
                            let btn = match evdev_button_from_code(key.0) {
                                Some(b) => b,
                                None => {
                                    reemit(vd, &[raw]);
                                    continue;
                                }
                            };
                            match callback(CaptureEvent::ButtonRelease(btn)) {
                                CaptureDecision::Passthrough => reemit(vd, &[raw]),
                                CaptureDecision::Suppress => {}
                            }
                        }
                        DeviceMsg::MouseMove { dx, dy, raw_x, raw_y } => {
                            CURSOR_X.fetch_add(dx, Ordering::Relaxed);
                            CURSOR_Y.fetch_add(dy, Ordering::Relaxed);
                            match callback(CaptureEvent::MouseMoveRel(dx, dy)) {
                                CaptureDecision::Passthrough => {
                                    let mut events: Vec<InputEvent> = Vec::with_capacity(3);
                                    if let Some(e) = raw_x { events.push(e); }
                                    if let Some(e) = raw_y { events.push(e); }
                                    if !events.is_empty() {
                                        reemit(vd, &events);
                                    }
                                }
                                CaptureDecision::Suppress => {}
                            }
                        }
                        DeviceMsg::Scroll { v, h, raw_v, raw_h } => {
                            if v != 0 {
                                match callback(CaptureEvent::Scroll(0, v)) {
                                    CaptureDecision::Passthrough => {
                                        if let Some(e) = raw_v { reemit(vd, &[e]); }
                                    }
                                    CaptureDecision::Suppress => {}
                                }
                            }
                            if h != 0 {
                                match callback(CaptureEvent::Scroll(h, 0)) {
                                    CaptureDecision::Passthrough => {
                                        if let Some(e) = raw_h { reemit(vd, &[e]); }
                                    }
                                    CaptureDecision::Suppress => {}
                                }
                            }
                        }
                        DeviceMsg::OtherBatch(events) => {
                            reemit(vd, &events);
                        }
                    }
                }
            })
            .ok();
    });
}

fn reemit(vd: &'static Mutex<VirtualDevice>, events: &[InputEvent]) {
    if events.is_empty() {
        return;
    }
    let mut all: Vec<InputEvent> = Vec::with_capacity(events.len() + 1);
    all.extend_from_slice(events);
    all.push(syn_event());
    if let Ok(mut vd) = vd.lock() {
        if let Err(e) = vd.emit(&all) {
            warn!("evdev re-emit error: {}", e);
        }
    }
}
