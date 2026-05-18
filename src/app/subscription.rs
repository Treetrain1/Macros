use crate::app::message::Message;
use crate::app::App;
use cosmic::iced::{keyboard, Subscription};

pub(crate) fn build_subscription(app: &App) -> Subscription<Message> {
    let key_sub = if app.editor_ui.is_capturing_key() {
        keyboard::listen().map(Message::KeyCaptureEvent)
    } else {
        Subscription::none()
    };

    Subscription::batch(vec![
        key_sub,
        Subscription::run(crate::app::hotkeys::queue::hotkey_sub),
    ])
}
