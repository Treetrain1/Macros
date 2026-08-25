//! Wire format for the shared-memory bridge between the embedded engine
//! (running inside GD, under Wine on Proton) and `macros-linux-bridge` (a
//! native Linux process that does real input capture/emission, since
//! Windows' `WH_KEYBOARD_LL`/`WH_MOUSE_LL` hooks don't see real host input
//! under Wine, and `SendInput`-based emission is unreliable there too).
//!
//! Always compiled (no platform gate) — both `macros-ffi` (Windows target)
//! and `macros-linux-bridge` (Linux target) depend on this module directly,
//! so the two ends can never disagree about the encoding.

use crate::input::types::{MacroButton, MacroKey};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};

/// Captured on the Linux side (real hardware input), consumed on the
/// Windows side and fed into `recording::build_capture_callback()` exactly
/// as if it came from a native OS hook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WireCaptureEvent {
    KeyPress(MacroKey),
    KeyRelease(MacroKey),
    ButtonPress(MacroButton),
    ButtonRelease(MacroButton),
    MouseMoveRel(i32, i32),
    MouseMoveAbs(f64, f64),
    Scroll(i32, i32),
}

/// A `CaptureTimestamp::Hardware` value can't cross the wire directly
/// (`SystemTime` isn't `#[repr(C)]`-portable) — split into seconds+nanos
/// since `UNIX_EPOCH` and reconstructed on the other side.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct WireTimestamp {
    pub secs: u64,
    pub nanos: u32,
}

impl WireTimestamp {
    pub fn from_system_time(t: std::time::SystemTime) -> Self {
        let d = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        Self { secs: d.as_secs(), nanos: d.subsec_nanos() }
    }

    pub fn to_system_time(self) -> std::time::SystemTime {
        std::time::UNIX_EPOCH + std::time::Duration::new(self.secs, self.nanos)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireCapture {
    pub event: WireCaptureEvent,
    pub ts: WireTimestamp,
}

impl From<WireCaptureEvent> for crate::macros::backend::CaptureEvent {
    fn from(e: WireCaptureEvent) -> Self {
        use crate::macros::backend::CaptureEvent as CE;
        match e {
            WireCaptureEvent::KeyPress(k) => CE::KeyPress(k),
            WireCaptureEvent::KeyRelease(k) => CE::KeyRelease(k),
            WireCaptureEvent::ButtonPress(b) => CE::ButtonPress(b),
            WireCaptureEvent::ButtonRelease(b) => CE::ButtonRelease(b),
            WireCaptureEvent::MouseMoveRel(dx, dy) => CE::MouseMoveRel(dx, dy),
            WireCaptureEvent::MouseMoveAbs(x, y) => CE::MouseMoveAbs(x, y),
            WireCaptureEvent::Scroll(h, v) => CE::Scroll(h, v),
        }
    }
}

/// Control-plane commands, sent Windows (Wine-hosted mod) → Linux
/// (`macros-linux-bridge`). Playback used to be paced entirely on the
/// Windows side: one `WireEmitCommand` shipped across per input event, the
/// instant `runner::run()`'s Wait-based deadline loop decided to fire it.
/// But that loop ran inside the Wine-hosted GD process, where Wine's
/// `SetThreadPriority` emulation is much weaker than real `SCHED_FIFO` —
/// under load, the deadline loop could get preempted for unpredictable
/// stretches, throwing macro timing off just enough to land inputs at the
/// wrong moment (confirmed: random deaths at different points in the same
/// macro run, never reproduced running the same macro through the old
/// standalone native-Linux desktop app talking to GD over a plain TCP
/// socket instead of this bridge).
///
/// Now the whole timed run happens natively on the Linux side instead —
/// only these two control commands cross the wire, never per-event
/// emission. `macros-linux-bridge` loads the macro itself (straight from
/// the OS-default config dir — the same file the Windows side's
/// `macros-dir` override setting reaches via its `Z:` mapping, so no macro
/// data needs to cross the wire either) and runs `macros_core::macros::
/// Macro::run` exactly as the old desktop app did, including its own
/// `raise_current_thread_priority()` call — on Linux that's the `#[cfg(unix)]`
/// branch, real `SCHED_FIFO`, not Wine's emulation of it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WireControlCommand {
    /// Run the macro with this id — fire-and-forget, mirroring
    /// `macros_run_macro`'s own "starts a background run, returns
    /// immediately" contract. The `f64` is `elapsed_overshoot_ms` from that
    /// same call: how much real time had already passed, before this
    /// command was even sent, since the moment playback was actually
    /// supposed to start (macros-gd's attempt-start trigger only fires once
    /// per game frame, so it always overshoots its own 200ms grace-period
    /// target by that frame's `dt`). `macros-linux-bridge` feeds it into
    /// `Macro::run_with_offset` so the run's first `Wait` deadline anchors
    /// to the *intended* start instant instead of whenever this command
    /// happens to get noticed and dispatched.
    RunMacro(String, f64),
    /// Stops every in-flight run started via `RunMacro`, mirroring
    /// `macros_stop_loop`.
    StopLoop,
}

/// Generous relative to a bincode-encoded `WireCapture`/`WireControlCommand`
/// (small enums over primitives/short strings, `RunMacro`'s id being the
/// biggest at one UUID-simple string) — actual encoded size checked in
/// tests below; this is deliberately far above it.
pub const SLOT_SIZE: usize = 128;
/// 512 turned out not to be enough headroom in practice — a real recording
/// session filled it (and started dropping events) within ~14 seconds
/// against a real mouse/keyboard, confirmed against an actual Proton/GD
/// session. 16384 costs ~2MB per ring (trivial) and gives roughly 30x the
/// margin at the same observed event rate.
pub const RING_CAPACITY: usize = 16384;

#[repr(C)]
pub struct RingSlot {
    pub len: u32,
    pub data: [u8; SLOT_SIZE - 4],
}

#[repr(C)]
pub struct RingBuffer {
    pub head: AtomicU32,
    pub tail: AtomicU32,
    /// Cross-process spinlock guarding `try_push` — needed because
    /// `macros-linux-bridge` spawns one reader thread *per input device*
    /// and every one of them pushes into the same capture ring directly.
    /// Without this, two threads can read the same `head`, write into the
    /// same slot, and only one push ends up counted — a real race
    /// confirmed against an actual Proton/GD session (recording worked
    /// inconsistently depending on whether multiple devices happened to
    /// produce events in the same narrow window). `try_pop` stays
    /// single-consumer-only (true on both ends: one capture-forwarder
    /// thread, one emit-loop thread), so it needs no such lock.
    push_lock: AtomicU32,
    pub slots: [RingSlot; RING_CAPACITY],
}

impl RingBuffer {
    /// Safe for multiple concurrent producers (see `push_lock`). Returns
    /// `false` if the ring is full (caller's responsibility to retry/back
    /// off).
    pub fn try_push(&self, bytes: &[u8]) -> bool {
        assert!(bytes.len() <= SLOT_SIZE - 4, "encoded message exceeds SLOT_SIZE");

        while self.push_lock.compare_exchange_weak(0, 1, Ordering::Acquire, Ordering::Relaxed).is_err() {
            std::hint::spin_loop();
        }

        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        if head.wrapping_sub(tail) as usize >= RING_CAPACITY {
            self.push_lock.store(0, Ordering::Release);
            return false;
        }
        let idx = (head as usize) % RING_CAPACITY;
        // SAFETY: `push_lock` above ensures only one producer is ever in
        // this block at a time, and the consumer only reads a slot after
        // observing `head` advance past it (Release below).
        unsafe {
            let slot = &self.slots[idx] as *const RingSlot as *mut RingSlot;
            (&raw mut (*slot).len).write(bytes.len() as u32);
            (&mut (*slot).data)[..bytes.len()].copy_from_slice(bytes);
        }
        self.head.store(head.wrapping_add(1), Ordering::Release);
        self.push_lock.store(0, Ordering::Release);
        true
    }

    /// Single-consumer only. Returns `None` if the ring is empty.
    pub fn try_pop(&self, out: &mut [u8; SLOT_SIZE - 4]) -> Option<usize> {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        if tail == head {
            return None;
        }
        let idx = (tail as usize) % RING_CAPACITY;
        let slot = &self.slots[idx];
        let len = slot.len as usize;
        out[..len].copy_from_slice(&slot.data[..len]);
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
        Some(len)
    }
}

#[repr(C)]
pub struct SharedRegion {
    /// Bumped by macros-ffi; macros-linux-bridge exits if this goes stale,
    /// so it never orphans itself if GD is killed/crashes.
    pub windows_heartbeat: AtomicU32,
    /// Bumped by macros-linux-bridge; lets macros-ffi notice the bridge
    /// process died (best-effort, not load-bearing in v1).
    pub linux_heartbeat: AtomicU32,
    /// Linux (producer) → Windows (consumer).
    pub capture: RingBuffer,
    /// Windows (producer) → Linux (consumer). Carries `WireControlCommand`s
    /// — was per-input-event `WireEmitCommand`s before playback timing
    /// moved to the Linux side; renamed along with that so the field name
    /// still describes what actually flows through it.
    pub control: RingBuffer,
}

pub const SHARED_REGION_SIZE: usize = std::mem::size_of::<SharedRegion>();

pub fn encode_capture(msg: &WireCapture) -> Vec<u8> {
    bincode::serde::encode_to_vec(msg, bincode::config::standard()).expect("WireCapture encode")
}

pub fn decode_capture(bytes: &[u8]) -> Option<WireCapture> {
    bincode::serde::decode_from_slice(bytes, bincode::config::standard()).ok().map(|(v, _)| v)
}

pub fn encode_control(msg: &WireControlCommand) -> Vec<u8> {
    bincode::serde::encode_to_vec(msg, bincode::config::standard()).expect("WireControlCommand encode")
}

pub fn decode_control(bytes: &[u8]) -> Option<WireControlCommand> {
    bincode::serde::decode_from_slice(bytes, bincode::config::standard()).ok().map(|(v, _)| v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::types::MacroKey;

    #[test]
    fn capture_roundtrips_and_fits_slot() {
        let msg = WireCapture {
            event: WireCaptureEvent::KeyPress(MacroKey::Unicode('x')),
            ts: WireTimestamp::from_system_time(std::time::SystemTime::now()),
        };
        let bytes = encode_capture(&msg);
        assert!(bytes.len() <= SLOT_SIZE - 4, "encoded WireCapture ({} bytes) exceeds slot capacity", bytes.len());
        let decoded = decode_capture(&bytes).unwrap();
        match decoded.event {
            WireCaptureEvent::KeyPress(MacroKey::Unicode('x')) => {}
            other => panic!("roundtrip mismatch: {other:?}"),
        }
    }

    #[test]
    fn control_roundtrips_and_fits_slot() {
        // A real macro id (uuid::Uuid::new_v4().simple()) is 32 hex chars —
        // exercises the actual worst-case length rather than a short literal.
        let id = "0123456789abcdef0123456789abcdef".to_string();
        let msg = WireControlCommand::RunMacro(id.clone(), 37.5);
        let bytes = encode_control(&msg);
        assert!(bytes.len() <= SLOT_SIZE - 4, "encoded WireControlCommand ({} bytes) exceeds slot capacity", bytes.len());
        let decoded = decode_control(&bytes).unwrap();
        match decoded {
            WireControlCommand::RunMacro(decoded_id, overshoot_ms) if decoded_id == id && overshoot_ms == 37.5 => {}
            other => panic!("roundtrip mismatch: {other:?}"),
        }
    }

    #[test]
    fn ring_buffer_push_pop() {
        // Boxed: RING_CAPACITY * SLOT_SIZE is too large for the test thread's stack.
        let ring: Box<RingBuffer> = unsafe {
            let layout = std::alloc::Layout::new::<RingBuffer>();
            let ptr = std::alloc::alloc_zeroed(layout) as *mut RingBuffer;
            Box::from_raw(ptr)
        };
        assert!(ring.try_push(b"hello"));
        let mut buf = [0u8; SLOT_SIZE - 4];
        let len = ring.try_pop(&mut buf).unwrap();
        assert_eq!(&buf[..len], b"hello");
        assert!(ring.try_pop(&mut buf).is_none());
    }

    /// Regression test for the multi-producer race: macros-linux-bridge
    /// spawns one reader thread per input device, and every one of them
    /// pushes into the same capture ring concurrently. A single-producer
    /// test can't catch this — it needs actual concurrent pushers to
    /// exercise the `push_lock`. Every pushed value must be popped exactly
    /// once, with its content intact (no torn/overwritten slots, no lost
    /// count in `head`).
    #[test]
    fn ring_buffer_survives_concurrent_producers() {
        // Boxed for the same reason as the test above (too large for the
        // stack); `thread::scope` can borrow it directly without needing
        // `'static`, unlike plain `thread::spawn`.
        let ring: Box<RingBuffer> = unsafe {
            let layout = std::alloc::Layout::new::<RingBuffer>();
            let ptr = std::alloc::alloc_zeroed(layout) as *mut RingBuffer;
            Box::from_raw(ptr)
        };

        const PRODUCERS: usize = 10;
        const PER_PRODUCER: usize = 500;

        std::thread::scope(|scope| {
            for p in 0..PRODUCERS {
                let ring = &ring;
                scope.spawn(move || {
                    for i in 0..PER_PRODUCER {
                        let msg = format!("p{p}-{i}");
                        while !ring.try_push(msg.as_bytes()) {
                            std::thread::yield_now();
                        }
                    }
                });
            }
        });

        let mut seen = std::collections::HashSet::new();
        let mut buf = [0u8; SLOT_SIZE - 4];
        for _ in 0..(PRODUCERS * PER_PRODUCER) {
            let len = ring.try_pop(&mut buf).expect("ring should have exactly PRODUCERS*PER_PRODUCER items, got fewer");
            let s = std::str::from_utf8(&buf[..len]).unwrap().to_string();
            assert!(seen.insert(s.clone()), "duplicate/corrupted entry popped: {s}");
        }
        assert!(ring.try_pop(&mut buf).is_none(), "ring had extra items beyond what was pushed");
        assert_eq!(seen.len(), PRODUCERS * PER_PRODUCER);
    }
}
