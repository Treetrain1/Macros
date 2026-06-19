use crate::app::message::Message;
use crate::recording::{self, QueueSignal};
use cosmic::iced::futures::{SinkExt, Stream};

pub(crate) fn hotkey_sub() -> impl Stream<Item = Message> {
    cosmic::iced::stream::channel(32, |mut sender: cosmic::iced::futures::channel::mpsc::Sender<Message>| async move {
        let mut rx = recording::take_queue_receiver();
        while let Some(signal) = rx.recv().await {
            let message = match signal {
                QueueSignal::Hotkey(action) => Message::ExecuteHotkeyAction(action),
                QueueSignal::Stop => Message::StopRecording,
            };
            let _ = sender.send(message).await;
        }
    })
}
