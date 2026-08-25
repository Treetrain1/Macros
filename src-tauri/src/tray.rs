use tauri::menu::MenuBuilder;
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Cef, Manager};

/// Builds the tray icon shown while "close to tray" is enabled: left click
/// opens the main window, right click shows the Open/Quit menu.
pub(crate) fn build(app: &AppHandle<Cef>) -> tauri::Result<TrayIcon<Cef>> {
    ensure_gtk_init(app);

    let menu = MenuBuilder::new(app).text("open", "Open").text("quit", "Quit").build()?;

    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Macros")
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(app)
}

pub(crate) fn show_main_window(app: &AppHandle<Cef>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// tray-icon's Linux backend (muda + libappindicator) needs GTK initialized
/// before it can build a menu. The wry runtime gets this for free from tao's
/// `EventLoop::new()`, but our CEF runtime's winit event loop never touches
/// GTK, so nothing else in the app will have called `gtk::init()` yet.
#[cfg(target_os = "linux")]
fn ensure_gtk_init(app: &AppHandle<Cef>) {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        let (tx, rx) = std::sync::mpsc::channel();
        if app
            .run_on_main_thread(move || {
                let _ = gtk::init();
                let _ = tx.send(());
            })
            .is_ok()
        {
            let _ = rx.recv();
        }
    });
}

#[cfg(not(target_os = "linux"))]
fn ensure_gtk_init(_app: &AppHandle<Cef>) {}
