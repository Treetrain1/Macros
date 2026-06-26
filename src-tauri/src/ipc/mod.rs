//! Loopback TCP control interface so external processes (e.g. a Geode mod
//! running Geometry Dash under Proton) can trigger recording/playback at a
//! precise moment, without going through a physical hotkey press. Commands
//! are translated 1:1 onto the existing hotkey `QueueSignal` pipeline so the
//! rest of the app behaves exactly as if the matching hotkey had been
//! pressed.

use crate::hotkey_types::HotkeyAction;
use crate::recording::{self, QueueSignal};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tracing::{info, warn};

#[derive(Deserialize)]
struct IpcRequest {
    cmd: String,
    id: Option<String>,
}

#[derive(Serialize)]
struct IpcResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pong: Option<bool>,
}

impl IpcResponse {
    fn ok() -> Self {
        IpcResponse { ok: true, error: None, pong: None }
    }

    fn pong() -> Self {
        IpcResponse { ok: true, error: None, pong: Some(true) }
    }

    fn error(message: impl Into<String>) -> Self {
        IpcResponse { ok: false, error: Some(message.into()), pong: None }
    }

    fn to_line(&self) -> String {
        let mut line = serde_json::to_string(self).unwrap_or_else(|_| "{\"ok\":false}".to_string());
        line.push('\n');
        line
    }
}

/// Binds a loopback-only listener and serves connections until `shutdown`
/// is set to `true`. Never binds beyond 127.0.0.1 — this interface trusts any
/// local process, matching the trust level already implied by the
/// global-hotkey grab (anything that can synthesize input locally can already
/// trigger it).
///
/// Each accepted connection is handled in its own spawned task, so `shutdown`
/// is also forwarded into those tasks (and not just the accept loop) —
/// otherwise stopping the server would only stop accepting *new* connections
/// while any already-connected client (e.g. a persistent IPC client) kept
/// being serviced indefinitely.
pub(crate) async fn run_server(port: u16, mut shutdown: watch::Receiver<bool>) {
    let listener = match TcpListener::bind(("127.0.0.1", port)).await {
        Ok(listener) => listener,
        Err(err) => {
            warn!("IPC: failed to bind 127.0.0.1:{port}: {err}");
            return;
        }
    };
    info!("IPC: listening on 127.0.0.1:{port}");

    loop {
        tokio::select! {
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((socket, _addr)) => {
                        tokio::spawn(handle_connection(socket, shutdown.clone()));
                    }
                    Err(err) => warn!("IPC: accept error: {err}"),
                }
            }
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    info!("IPC: server stopping");
                    return;
                }
            }
        }
    }
}

async fn handle_connection(socket: TcpStream, mut shutdown: watch::Receiver<bool>) {
    let (read_half, mut write_half) = socket.into_split();
    let mut lines = BufReader::new(read_half).lines();

    loop {
        tokio::select! {
            line_result = lines.next_line() => {
                let line = match line_result {
                    Ok(Some(line)) => line,
                    Ok(None) => return,
                    Err(err) => {
                        warn!("IPC: read error: {err}");
                        return;
                    }
                };
                if line.trim().is_empty() {
                    continue;
                }

                let response = handle_request(&line);
                if write_half.write_all(response.to_line().as_bytes()).await.is_err() {
                    return;
                }
            }
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    /// Regression test for the bug where "Stop Server" left already-connected
    /// clients (like the Geode mod's persistent connection) running forever,
    /// because aborting the accept loop's task doesn't touch the separately
    /// spawned per-connection tasks.
    #[tokio::test]
    async fn shutdown_closes_already_connected_clients() {
        let port = 58231;
        let (tx, rx) = watch::channel(false);
        let server = tokio::spawn(run_server(port, rx));
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        stream.write_all(b"{\"cmd\":\"ping\"}\n").await.unwrap();
        let mut buf = [0u8; 64];
        let n = stream.read(&mut buf).await.unwrap();
        assert!(n > 0, "expected a pong response before shutdown");

        tx.send(true).unwrap();

        let mut buf2 = [0u8; 64];
        let read = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            stream.read(&mut buf2),
        )
        .await
        .expect("connection was not closed after shutdown")
        .unwrap();
        assert_eq!(read, 0, "connection should be closed (EOF) after shutdown");

        server.abort();
    }
}

fn handle_request(line: &str) -> IpcResponse {
    let request: IpcRequest = match serde_json::from_str(line) {
        Ok(request) => request,
        Err(err) => return IpcResponse::error(format!("invalid json: {err}")),
    };

    match request.cmd.as_str() {
        "start_recording" => {
            recording::push_queue_signal(QueueSignal::Hotkey(HotkeyAction::StartRecordingImmediate));
            IpcResponse::ok()
        }
        "stop_recording" => {
            recording::push_queue_signal(QueueSignal::Stop);
            IpcResponse::ok()
        }
        "run_macro" => {
            let action = match request.id {
                Some(id) => HotkeyAction::RunSpecificMacro(id),
                None => HotkeyAction::RunMacro,
            };
            recording::push_queue_signal(QueueSignal::Hotkey(action));
            IpcResponse::ok()
        }
        "stop_loop" => {
            recording::push_queue_signal(QueueSignal::Hotkey(HotkeyAction::StopLoop));
            IpcResponse::ok()
        }
        "ping" => IpcResponse::pong(),
        other => IpcResponse::error(format!("unknown command: {other}")),
    }
}
