use std::io::Write;
use std::net::{TcpListener, TcpStream};

/// Loopback port used purely to detect a second launch and wake the running
/// instance -- distinct from the user-configurable macro-control IPC port
/// (default 47821) so the two don't collide under default settings.
const ACTIVATION_PORT: u16 = 47825;

/// Tries to claim the app's single-instance activation port.
///
/// - `Some(listener)`: nobody else is running; the caller owns the port and
///   should keep accepting connections on it for the rest of the app's life
///   to catch later launches.
/// - `None`: another instance already owns the port. It's been pinged to
///   show its window, so the caller should exit immediately without
///   building a second app instance (window, tray icon, etc.).
pub(crate) fn claim_or_activate_existing() -> Option<TcpListener> {
    match TcpListener::bind(("127.0.0.1", ACTIVATION_PORT)) {
        Ok(listener) => Some(listener),
        Err(_) => {
            if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", ACTIVATION_PORT)) {
                let _ = stream.write_all(b"show\n");
            }
            None
        }
    }
}
