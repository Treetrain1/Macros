use cosmic::iced::{keyboard, Subscription};
use crate::app::message::Message;
use crate::app::App;

pub(crate) fn build_subscription(app: &App) -> Subscription<Message> {
    let key_capture_subscription = if app.editor_ui.key_capture_index.is_some() {
        keyboard::listen().map(Message::KeyCaptureEvent)
    } else {
        Subscription::none()
    };

    #[cfg(not(target_os = "linux"))]
    {
        Subscription::batch(vec![
            Subscription::run(crate::app::hotkeys::non_linux::hotkey_sub),
            key_capture_subscription,
        ])
    }

    #[cfg(target_os = "linux")]
    {
        Subscription::batch(vec![
            key_capture_subscription,
            Subscription::run(crate::app::hotkeys::linux::macro_nav_sub),
        ])
    }
}
