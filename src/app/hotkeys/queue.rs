use crate::app::message::Message;
use crate::hotkey_types::HotkeyAction;
use crate::recording;
use cosmic::iced::futures::{SinkExt, Stream};

pub(crate) fn hotkey_sub() -> impl Stream<Item = Message> {
    cosmic::iced::stream::channel(
        32,
        |mut sender: cosmic::iced::futures::channel::mpsc::Sender<Message>| async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;

                let actions: Vec<HotkeyAction> = recording::get_hotkey_action_queue()
                    .try_lock()
                    .map(|mut q| q.drain(..).collect())
                    .unwrap_or_default();
                for action in actions {
                    let _ = sender.send(Message::ExecuteHotkeyAction(action)).await;
                }

                let stop_count = recording::get_stop_signal()
                    .try_lock()
                    .map(|mut q| q.drain(..).count())
                    .unwrap_or(0);
                for _ in 0..stop_count {
                    let _ = sender.send(Message::StopRecording).await;
                }
            }
        },
    )
}
