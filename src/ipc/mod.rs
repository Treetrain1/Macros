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

/// Binds a loopback-only listener and serves connections until the process
/// exits. Never binds beyond 127.0.0.1 — this interface trusts any local
/// process, matching the trust level already implied by the global-hotkey
/// grab (anything that can synthesize input locally can already trigger it).
pub(crate) async fn run_server(port: u16) {
    let listener = match TcpListener::bind(("127.0.0.1", port)).await {
        Ok(listener) => listener,
        Err(err) => {
            warn!("IPC: failed to bind 127.0.0.1:{port}: {err}");
            return;
        }
    };
    info!("IPC: listening on 127.0.0.1:{port}");

    loop {
        match listener.accept().await {
            Ok((socket, _addr)) => {
                tokio::spawn(handle_connection(socket));
            }
            Err(err) => warn!("IPC: accept error: {err}"),
        }
    }
}

async fn handle_connection(socket: TcpStream) {
    let (read_half, mut write_half) = socket.into_split();
    let mut lines = BufReader::new(read_half).lines();

    loop {
        let line = match lines.next_line().await {
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
