#![cfg(not(target_os = "linux"))]

use cosmic::iced::futures::SinkExt;
use cosmic::iced::futures::Stream;
use cosmic::iced::stream::channel;
use cosmic::iced::futures::channel::mpsc::Sender;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::{Code, HotKey, Modifiers}};
use crate::app::message::Message;
use crate::app::state::HotkeyState;

pub(crate) fn hotkey_sub() -> impl Stream<Item = Message> {
    channel(32, |mut sender: Sender<Message>| async move {
        let receiver = GlobalHotKeyEvent::receiver();
        loop {
            if let Ok(event) = receiver.try_recv() {
                if event.state == HotKeyState::Pressed {
                    sender
                        .send(Message::GlobalHotkeyEvent(event))
                        .await
                        .unwrap();
                }
            }
            async_std::task::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
}

pub(crate) fn setup_non_linux_hotkeys() -> HotkeyState {
    let manager = GlobalHotKeyManager::new().unwrap();
    let run_macro_hotkey = HotKey::new(Some(Modifiers::CONTROL), Code::F6);
    let stop_loop_hotkey = HotKey::new(Some(Modifiers::CONTROL), Code::F7);
    let run_macro_id = run_macro_hotkey.id();
    let stop_loop_id = stop_loop_hotkey.id();

    manager.register(run_macro_hotkey).expect("Failed to register 'start' keybind");
    manager.register(stop_loop_hotkey).expect("Failed to register 'stop' keybind");

    HotkeyState { manager, run_macro_id, stop_loop_id }
}
