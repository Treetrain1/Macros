use crate::input::types::{Axis, Direction, MacroButton, MacroKey};
use std::sync::{Arc, Mutex};

pub enum CaptureDecision {
    Passthrough,
    Suppress,
}

pub enum CaptureEvent {
    KeyPress(MacroKey),
    KeyRelease(MacroKey),
    ButtonPress(MacroButton),
    ButtonRelease(MacroButton),
    /// Relative mouse movement in pixels (dx, dy).
    MouseMoveRel(i32, i32),
    /// Absolute mouse position.
    MouseMoveAbs(f64, f64),
    /// Scroll ticks (horizontal, vertical).
    Scroll(i32, i32),
}

pub trait InputBackend: Send + 'static {
    fn key(&mut self, key: MacroKey, dir: Direction) -> Result<(), String>;
    fn raw_keycode(&mut self, keycode: u16, dir: Direction) -> Result<(), String>;
    fn button(&mut self, button: MacroButton, dir: Direction) -> Result<(), String>;
    fn move_mouse_rel(&mut self, dx: i32, dy: i32) -> Result<(), String>;
    fn move_mouse_abs(&mut self, x: i32, y: i32) -> Result<(), String>;
    fn scroll(&mut self, amount: i32, axis: Axis) -> Result<(), String>;
    fn text(&mut self, s: &str) -> Result<(), String>;
    fn cursor_pos(&self) -> Option<(i32, i32)>;
}

/// Start the global input capture thread for the current platform.
/// The callback is called for each input event and returns whether to suppress it.
pub fn start_capture(
    callback: Box<dyn FnMut(CaptureEvent) -> CaptureDecision + Send + 'static>,
) {
    #[cfg(target_os = "linux")]
    evdev::start_capture_thread(callback);

    #[cfg(windows)]
    windows::start_capture_thread(callback);

    #[cfg(target_os = "macos")]
    macos::start_capture_thread(callback);

    // Silence unused-variable warning on unsupported platforms.
    #[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
    {
        let _ = callback;
        tracing::warn!("Input capture is not supported on this platform.");
    }
}

/// Create the platform-specific input backend wrapped in `Arc<Mutex<dyn InputBackend>>`.
/// The coercion to the trait object happens here while we still hold the concrete type.
pub fn create_backend() -> Option<Arc<Mutex<dyn InputBackend>>> {
    #[cfg(target_os = "linux")]
    {
        match evdev::EvdevBackend::new() {
            Ok(b) => {
                let arc: Arc<Mutex<dyn InputBackend>> = Arc::new(Mutex::new(b));
                return Some(arc);
            }
            Err(e) => {
                tracing::warn!("Failed to create evdev backend: {}", e);
                return None;
            }
        }
    }

    #[cfg(windows)]
    {
        let b = windows::WinApiBackend::new();
        let arc: Arc<Mutex<dyn InputBackend>> = Arc::new(Mutex::new(b));
        return Some(arc);
    }

    #[cfg(target_os = "macos")]
    {
        let b = macos::MacosBackend::new();
        let arc: Arc<Mutex<dyn InputBackend>> = Arc::new(Mutex::new(b));
        return Some(arc);
    }

    #[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
    {
        tracing::warn!("No input backend available on this platform.");
        None
    }
}

#[cfg(target_os = "linux")]
pub(crate) mod evdev_mapping;
#[cfg(target_os = "linux")]
pub(crate) mod evdev;

#[cfg(windows)]
pub(crate) mod windows;

#[cfg(target_os = "macos")]
pub(crate) mod macos;
