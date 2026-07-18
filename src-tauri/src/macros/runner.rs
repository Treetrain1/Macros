use crate::input::types::{Coordinate, Direction, InputToken, MacroKey};
use crate::input::value::Value;
use crate::macros::backend::{create_backend, InputBackend};
use crate::macros::priority::raise_current_thread_priority;
use crate::macros::{Instruction, Macro, Strand};
use spin_sleep::{SpinSleeper, SpinStrategy};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::warn;

static EMULATOR_FAILED: AtomicBool = AtomicBool::new(false);

pub(crate) fn emulator_failed() -> bool {
    EMULATOR_FAILED.load(Ordering::Relaxed)
}

fn spin_sleeper() -> &'static SpinSleeper {
    static SLEEPER: OnceLock<SpinSleeper> = OnceLock::new();
    SLEEPER.get_or_init(|| SpinSleeper::default().with_spin_strategy(SpinStrategy::SpinLoopHint))
}

/// Polling granularity while waiting for a `Wait` instruction to elapse during
/// a stoppable (loop-mode) run, so a stop signal lands quickly instead of
/// only being noticed once the full wait has elapsed.
const STOP_POLL_INTERVAL: Duration = Duration::from_millis(15);

impl Macro {
    /// Runs every "When Ran" entry-point strand (one whose first instruction
    /// is `Instruction::WhenRan`) concurrently, each on its own thread, all
    /// sharing the same `stop_flag` — so stopping a run stops every strand at
    /// once. Strands that aren't entry points are preserved on the macro but
    /// stay inert. Blocks until every entry strand has finished (or been
    /// stopped), matching the old single-root behavior for callers.
    pub(crate) fn run(self, emulator: Arc<Mutex<dyn InputBackend>>, stop_flag: Option<Arc<Mutex<bool>>>, speed_multiplier: f64) {
        let entry_strands: Vec<Vec<Instruction>> = self.strands.into_iter()
            .filter(Strand::starts_with_when_ran)
            .map(|s| s.instructions)
            .collect();

        let mut iter = entry_strands.into_iter();
        let Some(first) = iter.next() else { return };
        let rest: Vec<_> = iter.collect();

        if rest.is_empty() {
            run_instructions(first, emulator, stop_flag, speed_multiplier);
            return;
        }

        std::thread::scope(|scope| {
            for instructions in rest {
                let emulator = Arc::clone(&emulator);
                let stop_flag = stop_flag.clone();
                scope.spawn(move || run_instructions(instructions, emulator, stop_flag, speed_multiplier));
            }
            run_instructions(first, emulator, stop_flag, speed_multiplier);
        });
    }
}

/// `stop_flag`, when present, is checked between every instruction (and
/// while a `Wait` is elapsing) so the run can be aborted early — e.g. when
/// the "Stop Loop" hotkey is pressed mid-iteration. Either way, any keys
/// pressed so far are released before returning, whether the run finished
/// naturally or was aborted.
///
/// `speed_multiplier` scales every `Wait` instruction's duration
/// inversely — 2.0 runs waits at half their length (twice as
/// fast), 0.5 runs them at double length (half as fast) — so it behaves
/// like an actual playback speed rather than a wait-length multiplier. The
/// caller combines the macro's own multiplier with the global runtime
/// override into this single factor before calling in.
pub(crate) fn run_instructions(
    instructions: Vec<Instruction>,
    emulator: Arc<Mutex<dyn InputBackend>>,
    stop_flag: Option<Arc<Mutex<bool>>>,
    speed_multiplier: f64,
) {
    raise_current_thread_priority();
    let mut deadline = Instant::now();
    let mut pressed_keys: Vec<MacroKey> = Vec::new();

    let stop_requested = |flag: &Arc<Mutex<bool>>| -> bool {
        !flag.lock().map(|g| *g).unwrap_or(false)
    };

    let normalize_modifier_key = |key: MacroKey| -> MacroKey {
        match key {
            MacroKey::Shift => MacroKey::LShift,
            _ => key,
        }
    };

    let shift_is_pressed = |keys: &[MacroKey]| -> bool {
        keys.iter()
            .any(|k| matches!(k, MacroKey::Shift | MacroKey::LShift | MacroKey::RShift))
    };

    for ins in instructions {
        if let Some(flag) = &stop_flag {
            if stop_requested(flag) {
                break;
            }
        }

        #[allow(unreachable_patterns)]
        match ins {
            Instruction::Comment(_) => {}
            Instruction::WhenRan => {}
            Instruction::Wait(duration) => {
                let duration = match duration.eval_number() {
                    Ok(v) => v,
                    Err(e) => {
                        warn!("Skipping Wait: duration {}", e);
                        continue;
                    }
                };
                // Clamped rather than trusted as non-negative: `duration` may
                // be an `Op::Random` node whose lower bound sits below zero
                // (e.g. a jitter range wider than the base wait), and
                // `Duration::from_secs_f64` panics on a negative input.
                let actual = (duration / speed_multiplier).max(0.0);
                deadline += Duration::from_secs_f64(actual / 1000.0);

                if let Some(flag) = &stop_flag {
                    let mut stopped = false;
                    loop {
                        if stop_requested(flag) {
                            stopped = true;
                            break;
                        }
                        let now = Instant::now();
                        if now >= deadline {
                            break;
                        }
                        let remaining = deadline - now;
                        if remaining > STOP_POLL_INTERVAL {
                            spin_sleeper().sleep(STOP_POLL_INTERVAL);
                        } else {
                            spin_sleeper().sleep_until(deadline);
                            break;
                        }
                    }
                    if stopped {
                        break;
                    }
                } else {
                    let now = Instant::now();
                    if now >= deadline {
                        deadline = now; // fell behind; re-anchor instead of catching up
                    } else {
                        spin_sleeper().sleep_until(deadline);
                    }
                }
            }
            Instruction::Command(command) => {
                println!("Running command: {command}");
                if let Err(e) = Command::new("bash").args(["-c", &command]).status() {
                    warn!("Command failed: {}", e)
                }
            }
            Instruction::Token(token) => {
                let mut em = match emulator.lock() {
                    Ok(guard) => guard,
                    Err(err) => {
                        warn!("Failed to lock emulator mutex: {}", err);
                        return;
                    }
                };

                match token {
                    InputToken::Text(text) => {
                        if let Err(err) = em.text(&text) {
                            warn!("Failed to type text '{}': {}", text, err);
                        }
                    }
                    InputToken::Key(key, direction) => {
                        let normalized_key = normalize_modifier_key(key);
                        let key_for_event =
                            match (normalized_key.clone(), direction.clone()) {
                                (MacroKey::Unicode(c), Direction::Click)
                                    if shift_is_pressed(&pressed_keys)
                                        && c.is_ascii_lowercase() =>
                                {
                                    MacroKey::Unicode(c.to_ascii_uppercase())
                                }
                                (key, _) => key,
                            };

                        match em.key(key_for_event, direction.clone()) {
                            Ok(()) => match direction {
                                Direction::Press => {
                                    if !pressed_keys.contains(&normalized_key) {
                                        pressed_keys.push(normalized_key);
                                    }
                                }
                                Direction::Release => {
                                    pressed_keys.retain(|k| k != &normalized_key);
                                }
                                Direction::Click => {}
                            },
                            Err(err) => {
                                warn!(
                                    "Failed to press key {:?} ({:?}): {}",
                                    normalized_key, direction, err
                                );
                            }
                        }
                    }
                    InputToken::Raw(keycode, direction) => {
                        if let Err(err) = em.raw_keycode(keycode, direction) {
                            warn!("Failed to emit raw keycode {}: {}", keycode, err);
                        }
                    }
                    InputToken::Button(button, direction) => {
                        let dir_clone = direction.clone();
                        if let Err(err) = em.button(button.clone(), direction) {
                            warn!(
                                "Failed to click button {:?} ({:?}): {}",
                                button, dir_clone, err
                            );
                        }
                    }
                    InputToken::MoveMouse(x, y, Coordinate::Rel) => {
                        let x = match x.eval_number() {
                            Ok(v) => v.round() as i32,
                            Err(e) => { warn!("Skipping relative mouse move: x {}", e); continue; }
                        };
                        let y = match y.eval_number() {
                            Ok(v) => v.round() as i32,
                            Err(e) => { warn!("Skipping relative mouse move: y {}", e); continue; }
                        };
                        if let Err(err) = em.move_mouse_rel(x, y) {
                            warn!("Failed to move mouse rel ({}, {}): {}", x, y, err);
                        }
                    }
                    InputToken::MoveMouse(x, y, Coordinate::Abs) => {
                        let x = match x.eval_number() {
                            Ok(v) => v.round() as i32,
                            Err(e) => { warn!("Skipping absolute mouse move: x {}", e); continue; }
                        };
                        let y = match y.eval_number() {
                            Ok(v) => v.round() as i32,
                            Err(e) => { warn!("Skipping absolute mouse move: y {}", e); continue; }
                        };
                        if let Err(err) = em.move_mouse_abs(x, y) {
                            warn!("Failed to move mouse abs ({}, {}): {}", x, y, err);
                        }
                    }
                    InputToken::Scroll(amount, axis) => {
                        let amount = match amount.eval_number() {
                            Ok(v) => v.round() as i32,
                            Err(e) => { warn!("Skipping scroll: {}", e); continue; }
                        };
                        if let Err(err) = em.scroll(amount, axis) {
                            warn!("Failed to scroll by {}: {}", amount, err);
                        }
                    }
                }
            }
            _ => {
                warn!("Instruction not implemented.");
            }
        }
    }

    if !pressed_keys.is_empty() {
        if let Ok(mut em) = emulator.lock() {
            for key in pressed_keys.into_iter().rev() {
                if let Err(err) = em.key(key.clone(), Direction::Release) {
                    warn!("Failed to release key {:?} during cleanup: {}", key, err);
                }
            }
        } else {
            warn!("Failed to lock emulator mutex for key cleanup");
        }
    }
}

pub fn make_backend() -> Option<Arc<Mutex<dyn InputBackend>>> {
    match create_backend() {
        Some(b) => Some(b),
        None => {
            EMULATOR_FAILED.store(true, Ordering::Relaxed);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::types::{Axis, Direction, MacroButton, MacroKey};
    use crate::macros::Strand;

    struct NoopBackend;
    impl InputBackend for NoopBackend {
        fn key(&mut self, _key: MacroKey, _dir: Direction) -> Result<(), String> { Ok(()) }
        fn raw_keycode(&mut self, _keycode: u16, _dir: Direction) -> Result<(), String> { Ok(()) }
        fn button(&mut self, _button: MacroButton, _dir: Direction) -> Result<(), String> { Ok(()) }
        fn move_mouse_rel(&mut self, _dx: i32, _dy: i32) -> Result<(), String> { Ok(()) }
        fn move_mouse_abs(&mut self, _x: i32, _y: i32) -> Result<(), String> { Ok(()) }
        fn scroll(&mut self, _amount: i32, _axis: Axis) -> Result<(), String> { Ok(()) }
        fn text(&mut self, _s: &str) -> Result<(), String> { Ok(()) }
        fn cursor_pos(&self) -> Option<(i32, i32)> { None }
    }

    fn when_ran_strand(id: &str, wait_ms: f64) -> Strand {
        Strand {
            id: id.to_string(),
            x: 0,
            y: 0,
            instructions: vec![Instruction::WhenRan, Instruction::Wait(Value::number(wait_ms))],
        }
    }

    /// Three entry strands each waiting 150ms should finish in well under
    /// 3×150ms if they're actually running concurrently rather than one
    /// after another.
    #[test]
    fn run_executes_all_when_ran_strands_concurrently() {
        let mac = Macro {
            id: "m".into(),
            name: "Concurrent".into(),
            description: "".into(),
            strands: vec![
                when_ran_strand("a", 150.0),
                when_ran_strand("b", 150.0),
                when_ran_strand("c", 150.0),
                Strand { id: "inert".into(), x: 0, y: 0, instructions: vec![Instruction::Wait(Value::number(150.0))] },
            ],
            recording_target: None,
            speed_multiplier: 1.0,
            floating_values: vec![],
        };
        let emulator: Arc<Mutex<dyn InputBackend>> = Arc::new(Mutex::new(NoopBackend));
        let start = Instant::now();
        mac.run(emulator, None, 1.0);
        assert!(start.elapsed() < Duration::from_millis(400), "entry strands ran sequentially instead of concurrently");
    }

    /// A stop flag flipped false mid-run should cut every concurrently
    /// running entry strand short, not just one of them.
    #[test]
    fn stop_flag_stops_every_concurrent_strand() {
        let long_wait = 5_000.0;
        let mac = Macro {
            id: "m".into(),
            name: "Stoppable".into(),
            description: "".into(),
            strands: vec![when_ran_strand("a", long_wait), when_ran_strand("b", long_wait)],
            recording_target: None,
            speed_multiplier: 1.0,
            floating_values: vec![],
        };
        let emulator: Arc<Mutex<dyn InputBackend>> = Arc::new(Mutex::new(NoopBackend));
        let stop_flag = Arc::new(Mutex::new(true));
        let flag_clone = Arc::clone(&stop_flag);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            *flag_clone.lock().unwrap() = false;
        });
        let start = Instant::now();
        mac.run(emulator, Some(stop_flag), 1.0);
        assert!(start.elapsed() < Duration::from_millis(1000), "stop flag didn't stop both concurrent strands promptly");
    }
}
