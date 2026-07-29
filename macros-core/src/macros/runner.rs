use crate::input::types::{Coordinate, Direction, InputToken, MacroButton, MacroKey};
use crate::input::value::{Evaluated, Value};
use crate::macros::backend::{create_backend, InputBackend};
use crate::macros::priority::raise_current_thread_priority;
use crate::macros::{Instruction, Macro};
use spin_sleep::{SpinSleeper, SpinStrategy};
use std::collections::HashMap;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::warn;

/// Shared, macro-wide variable store — `AppState::variable_values` is the
/// UI/persistence side of this same map; execution threads share it via this
/// `Arc` so a `Set`/`Change` in one strand is visible to another running
/// concurrently. See `Macro::run`.
pub type VariableStore = Arc<Mutex<HashMap<String, Evaluated>>>;

/// A custom block's runtime shape — just enough to invoke it: its declared
/// input names (in prototype order, the positional key `args` line up
/// against) and its body instructions (everything after its `BlockHeader`).
/// Built once per run from `Macro::block_defs` + each block's header strand
/// (see `Macro::run`), then shared read-only via `ExecCtx::block_table` —
/// definitions are fixed for the duration of one run, same as the rest of
/// the instruction list.
pub struct BlockRuntime {
    pub input_names: Vec<String>,
    pub body: Vec<Instruction>,
}

/// Custom-block call depth guard — the only protection against runaway
/// self-/mutually-recursive blocks. Recursion itself is just plain Rust
/// function calls (`run_block`/`resolve_calls_and_params` calling back into
/// each other), so without this a cycle would eventually stack-overflow the
/// whole process instead of failing cleanly for just that one run.
/// Conservative rather than generous: each level of recursion here crosses
/// several real Rust stack frames (`resolve_calls_and_params` ->
/// `call_block` -> `run_block` -> back into `resolve_calls_and_params` for
/// the callee's own `Return`), and execution threads don't get an enlarged
/// stack (see the tauri app's `macros_thread` module and its plain
/// `thread::Builder::new().spawn`) — a
/// higher limit risks a real stack overflow (aborting the whole process)
/// before this check ever gets a chance to return a clean `Err`.
const MAX_CALL_DEPTH: u32 = 64;

/// Everything one strand's execution needs, threaded by `&mut` through
/// `run_block` and its recursive custom-block calls. `pressed_keys` is
/// shared for the *whole* strand's run (including every nested call) so
/// cleanup only ever happens once, at the outermost level (see
/// `run_strand`) — matching the pre-custom-blocks behavior where a strand's
/// run was a single flat instruction list with one release-on-exit pass.
/// `param_env` is the one field that's swapped out (saved/restored around a
/// nested call — see `call_block`) rather than shared, since each
/// invocation's bound parameters must stay isolated from its caller's and
/// from any concurrent/sibling invocation of the same block.
struct ExecCtx<'a> {
    emulator: Arc<Mutex<dyn InputBackend>>,
    stop_flag: Option<Arc<Mutex<bool>>>,
    speed_multiplier: f64,
    variables: VariableStore,
    block_table: Arc<HashMap<String, BlockRuntime>>,
    pressed_keys: &'a mut Vec<MacroKey>,
    /// Mouse buttons held by this strand, tracked and cleaned up exactly like
    /// `pressed_keys`. Without this, stopping a run between a button's press
    /// and its release leaves that button physically stuck down for the rest
    /// of the session — the pointer drags across every window and the machine
    /// looks broken, which reads as "the macro is still running".
    pressed_buttons: &'a mut Vec<MacroButton>,
    param_env: HashMap<String, Evaluated>,
}

impl<'a> ExecCtx<'a> {
    /// Resolves `value` all the way down to a tree `Value::eval()` can
    /// safely consume: macro-wide `Var` reads first (pure, tolerates a
    /// poisoned lock by falling back to an empty environment), then this
    /// invocation's `Param` reads and any `Call` nodes (which may run real
    /// instructions — see `resolve_calls_and_params`). Every instruction
    /// field's value resolution funnels through this one method.
    fn resolve(&mut self, value: &Value, depth: u32) -> Result<Value, String> {
        let vars_resolved = match self.variables.lock() {
            Ok(guard) => value.resolve_vars(&guard),
            Err(_) => value.resolve_vars(&HashMap::new()),
        };
        resolve_calls_and_params(&vars_resolved, self, depth)
    }
}

/// Walks a `Var`-resolved tree substituting `Param` leaves (from `ctx`'s
/// current invocation scope) and `Call` nodes (by actually running the
/// callee's body and capturing its `Return`) — the impure counterpart to
/// `Value::resolve_vars`, kept here rather than in `input/value.rs` because
/// it needs `run_block`/`call_block`'s execution capability, which that
/// module deliberately doesn't have (see `Value::eval`'s `Param`/`Call`
/// error arms — this is the *only* place allowed to resolve them away
/// first).
fn resolve_calls_and_params(value: &Value, ctx: &mut ExecCtx, depth: u32) -> Result<Value, String> {
    match value {
        Value::Number { value } => Ok(Value::Number { value: *value }),
        Value::Text { value } => Ok(Value::Text { value: value.clone() }),
        // Shouldn't appear here (resolve_vars already ran), but a harmless
        // passthrough rather than a hard error keeps this function total.
        Value::Var { name } => Ok(Value::Var { name: name.clone() }),
        Value::Param { name } => Ok(match ctx.param_env.get(name) {
            Some(Evaluated::Number(n)) => Value::Number { value: *n },
            Some(Evaluated::Text(s)) => Value::Text { value: s.clone() },
            None => Value::number(0.0),
        }),
        Value::Op { op, args, saved } => {
            let args = args.iter().map(|a| resolve_calls_and_params(a, ctx, depth)).collect::<Result<Vec<_>, _>>()?;
            Ok(Value::Op { op: *op, args, saved: saved.clone() })
        }
        Value::Call { block_id, args, .. } => {
            // Args are evaluated in the *caller's* current scope (ctx's
            // param_env as it is right now), fully resolved down to concrete
            // values before `call_block` swaps in the callee's own scope.
            let mut evaluated_args = Vec::with_capacity(args.len());
            for a in args {
                let resolved = resolve_calls_and_params(a, ctx, depth)?;
                evaluated_args.push(resolved.eval()?);
            }
            match call_block(block_id, evaluated_args, ctx, depth + 1)? {
                Some(Evaluated::Number(n)) => Ok(Value::number(n)),
                Some(Evaluated::Text(s)) => Ok(Value::Text { value: s }),
                None => Err(format!("custom block '{block_id}' didn't return a value")),
            }
        }
    }
}

/// Runs custom block `block_id`'s body with `arg_values` bound to its
/// declared inputs, returning whatever `Return` produced (`None` if the
/// body ran to completion without one — a well-formed `returns_value: true`
/// block should always `Return`, but a `CallBlock`/`returns_value: false`
/// invocation legitimately never does). Shared by both call positions:
/// `resolve_calls_and_params`'s `Value::Call` arm (value position) and
/// `run_block`'s own `Instruction::CallBlock` arm (command position, which
/// just discards the `Option`).
fn call_block(block_id: &str, arg_values: Vec<Evaluated>, ctx: &mut ExecCtx, depth: u32) -> Result<Option<Evaluated>, String> {
    if depth > MAX_CALL_DEPTH {
        return Err("custom block call depth exceeded (possible infinite recursion)".to_string());
    }
    // Cloning the Arc (not the table) sidesteps any borrow-checker
    // entanglement between reading `ctx.block_table` and later mutably
    // swapping `ctx.param_env` for the nested call.
    let block_table = Arc::clone(&ctx.block_table);
    let runtime = block_table.get(block_id).ok_or_else(|| format!("call to unknown custom block '{block_id}'"))?;
    let new_env: HashMap<String, Evaluated> = runtime.input_names.iter().cloned().zip(arg_values).collect();
    let saved_env = std::mem::replace(&mut ctx.param_env, new_env);
    let result = run_block(&runtime.body, ctx, depth);
    ctx.param_env = saved_env;
    result
}

static EMULATOR_FAILED: AtomicBool = AtomicBool::new(false);

pub fn emulator_failed() -> bool {
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
    /// stay inert, *except* a custom block's header strand, whose body is
    /// extracted into the shared `block_table` (built once, up front) rather
    /// than run directly — it only ever executes via `CallBlock`/`Value::Call`.
    /// Blocks until every entry strand has finished (or been stopped),
    /// matching the old single-root behavior for callers.
    pub fn run(
        self,
        emulator: Arc<Mutex<dyn InputBackend>>,
        stop_flag: Option<Arc<Mutex<bool>>>,
        speed_multiplier: f64,
        variables: VariableStore,
    ) {
        let block_defs = self.block_defs;
        let mut block_table: HashMap<String, BlockRuntime> = HashMap::new();
        let mut entry_strands: Vec<Vec<Instruction>> = Vec::new();

        for strand in self.strands {
            match strand.instructions.first() {
                Some(Instruction::BlockHeader(id)) => {
                    if let Some(def) = block_defs.iter().find(|b| &b.id == id) {
                        let input_names: Vec<String> = def.input_names().map(str::to_string).collect();
                        let body = strand.instructions[1..].to_vec();
                        block_table.insert(id.clone(), BlockRuntime { input_names, body });
                    }
                }
                _ if strand.starts_with_when_ran() => entry_strands.push(strand.instructions),
                _ => {}
            }
        }
        let block_table = Arc::new(block_table);

        let mut iter = entry_strands.into_iter();
        let Some(first) = iter.next() else { return };
        let rest: Vec<_> = iter.collect();

        if rest.is_empty() {
            run_strand(first, emulator, stop_flag, speed_multiplier, variables, block_table);
            return;
        }

        std::thread::scope(|scope| {
            for instructions in rest {
                let emulator = Arc::clone(&emulator);
                let stop_flag = stop_flag.clone();
                let variables = Arc::clone(&variables);
                let block_table = Arc::clone(&block_table);
                scope.spawn(move || run_strand(instructions, emulator, stop_flag, speed_multiplier, variables, block_table));
            }
            run_strand(first, emulator, stop_flag, speed_multiplier, variables, block_table);
        });
    }
}

/// Per-thread entry point for one strand's run: sets up a fresh `ExecCtx`
/// (empty `param_env`, its own `pressed_keys`) and runs the strand's
/// instructions top to bottom via `run_block`, releasing any keys still held
/// afterward — whether the strand finished naturally or was stopped, and
/// regardless of how deep any custom-block calls nested along the way, since
/// `pressed_keys` is shared through the whole call tree. The top-level
/// `Result`/`Option` `run_block` returns is discarded: a `Return` reaching
/// all the way out to strand level (rather than being caught by an
/// enclosing `call_block`) has nothing left to hand the value to, and a
/// resolve error here has already been `warn!`-logged at its source.
fn run_strand(
    instructions: Vec<Instruction>,
    emulator: Arc<Mutex<dyn InputBackend>>,
    stop_flag: Option<Arc<Mutex<bool>>>,
    speed_multiplier: f64,
    variables: VariableStore,
    block_table: Arc<HashMap<String, BlockRuntime>>,
) {
    raise_current_thread_priority();
    let mut pressed_keys: Vec<MacroKey> = Vec::new();
    let mut pressed_buttons: Vec<MacroButton> = Vec::new();
    {
        let mut ctx = ExecCtx {
            emulator: Arc::clone(&emulator),
            stop_flag,
            speed_multiplier,
            variables,
            block_table,
            pressed_keys: &mut pressed_keys,
            pressed_buttons: &mut pressed_buttons,
            param_env: HashMap::new(),
        };
        let _ = run_block(&instructions, &mut ctx, 0);
    }

    if !pressed_keys.is_empty() || !pressed_buttons.is_empty() {
        if let Ok(mut em) = emulator.lock() {
            for key in pressed_keys.into_iter().rev() {
                if let Err(err) = em.key(key.clone(), Direction::Release) {
                    warn!("Failed to release key {:?} during cleanup: {}", key, err);
                }
            }
            for button in pressed_buttons.into_iter().rev() {
                if let Err(err) = em.button(button.clone(), Direction::Release) {
                    warn!("Failed to release button {:?} during cleanup: {}", button, err);
                }
            }
        } else {
            warn!("Failed to lock emulator mutex for input cleanup");
        }
    }
}

/// Convenience entry point equivalent to `run_strand` with an empty
/// `block_table` — kept for callers/tests that don't need custom blocks, so
/// they can construct an `ExecCtx` without caring about the concept.
pub fn run_instructions(
    instructions: Vec<Instruction>,
    emulator: Arc<Mutex<dyn InputBackend>>,
    stop_flag: Option<Arc<Mutex<bool>>>,
    speed_multiplier: f64,
    variables: VariableStore,
) {
    run_strand(instructions, emulator, stop_flag, speed_multiplier, variables, Arc::new(HashMap::new()));
}

/// Runs `instructions` top to bottom against `ctx`, returning `Ok(Some(v))`
/// the instant a `Return` is hit (unwinding out of this call — and only this
/// call, via the early `return` below — back to whoever invoked it: either
/// `call_block`, for a nested custom-block call, or `run_strand`, at the
/// outermost level), `Ok(None)` if it runs to completion, or `Err` if
/// resolving/evaluating a value along the way failed hard enough to abort
/// (a call-depth overrun, or an inner `Return`'s own value failing to
/// resolve) — most per-instruction resolve errors are non-fatal and instead
/// `warn!`-log + skip that one instruction, exactly like before custom
/// blocks existed.
///
/// `stop_flag`, when present, is checked between every instruction (and
/// while a `Wait` is elapsing) so the run can be aborted early — e.g. when
/// the "Stop Loop" hotkey is pressed mid-iteration.
///
/// `speed_multiplier` scales every `Wait` instruction's duration
/// inversely — 2.0 runs waits at half their length (twice as
/// fast), 0.5 runs them at double length (half as fast) — so it behaves
/// like an actual playback speed rather than a wait-length multiplier. The
/// caller combines the macro's own multiplier with the global runtime
/// override into this single factor before calling in.
fn run_block(instructions: &[Instruction], ctx: &mut ExecCtx, depth: u32) -> Result<Option<Evaluated>, String> {
    if depth > MAX_CALL_DEPTH {
        return Err("custom block call depth exceeded (possible infinite recursion)".to_string());
    }

    // Anchors this call's own `Wait` pacing — deliberately local to this one
    // invocation (not threaded through `ctx`), so a nested call's waits pace
    // against each other independently; once it returns, the caller's own
    // (now possibly stale) deadline just falls behind and re-anchors on its
    // next `Wait`, the same "fell behind" handling already below.
    let mut deadline = Instant::now();

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
        if let Some(flag) = &ctx.stop_flag {
            if stop_requested(flag) {
                break;
            }
        }

        match ins {
            Instruction::Comment(_) => {}
            Instruction::WhenRan => {}
            Instruction::BlockHeader(_) => {}
            Instruction::Return(value) => {
                let resolved = ctx.resolve(value, depth)?;
                return Ok(Some(resolved.eval()?));
            }
            Instruction::CallBlock { block_id, args } => {
                let mut evaluated_args = Vec::with_capacity(args.len());
                for a in args {
                    match ctx.resolve(a, depth).and_then(|v| v.eval()) {
                        Ok(v) => evaluated_args.push(v),
                        Err(e) => {
                            warn!("Skipping call to '{}': arg {}", block_id, e);
                            continue;
                        }
                    }
                }
                // A well-formed `returns_value: false` block's body has no
                // `Return`; if one somehow sneaks in, it just ends this
                // nested call early rather than the value going anywhere.
                let _ = call_block(block_id, evaluated_args, ctx, depth + 1)?;
            }
            Instruction::Wait(duration) => {
                let duration = match ctx.resolve(duration, depth).and_then(|v| v.eval_number()) {
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
                let actual = (duration / ctx.speed_multiplier).max(0.0);
                deadline += Duration::from_secs_f64(actual / 1000.0);

                if let Some(flag) = &ctx.stop_flag {
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
                if let Err(e) = Command::new("bash").args(["-c", command]).status() {
                    warn!("Command failed: {}", e)
                }
            }
            Instruction::Token(token) => match token {
                InputToken::Text(value) => {
                    let text = match ctx.resolve(value, depth).and_then(|v| v.eval_text()) {
                        Ok(t) => t,
                        Err(e) => {
                            warn!("Skipping Text: {}", e);
                            continue;
                        }
                    };
                    let mut em = match ctx.emulator.lock() {
                        Ok(guard) => guard,
                        Err(err) => {
                            warn!("Failed to lock emulator mutex: {}", err);
                            return Ok(None);
                        }
                    };
                    if let Err(err) = em.text(&text) {
                        warn!("Failed to type text '{}': {}", text, err);
                    }
                }
                InputToken::Key(key, direction) => {
                    let mut em = match ctx.emulator.lock() {
                        Ok(guard) => guard,
                        Err(err) => {
                            warn!("Failed to lock emulator mutex: {}", err);
                            return Ok(None);
                        }
                    };
                    let normalized_key = normalize_modifier_key(key.clone());
                    let key_for_event = match (normalized_key.clone(), direction.clone()) {
                        (MacroKey::Unicode(c), Direction::Click)
                            if shift_is_pressed(ctx.pressed_keys) && c.is_ascii_lowercase() =>
                        {
                            MacroKey::Unicode(c.to_ascii_uppercase())
                        }
                        (key, _) => key,
                    };

                    match em.key(key_for_event, direction.clone()) {
                        Ok(()) => match direction {
                            Direction::Press => {
                                if !ctx.pressed_keys.contains(&normalized_key) {
                                    ctx.pressed_keys.push(normalized_key);
                                }
                            }
                            Direction::Release => {
                                ctx.pressed_keys.retain(|k| k != &normalized_key);
                            }
                            Direction::Click => {}
                        },
                        Err(err) => {
                            warn!("Failed to press key {:?} ({:?}): {}", normalized_key, direction, err);
                        }
                    }
                }
                InputToken::Raw(keycode, direction) => {
                    let mut em = match ctx.emulator.lock() {
                        Ok(guard) => guard,
                        Err(err) => {
                            warn!("Failed to lock emulator mutex: {}", err);
                            return Ok(None);
                        }
                    };
                    if let Err(err) = em.raw_keycode(*keycode, direction.clone()) {
                        warn!("Failed to emit raw keycode {}: {}", keycode, err);
                    }
                }
                InputToken::Button(button, direction) => {
                    let mut em = match ctx.emulator.lock() {
                        Ok(guard) => guard,
                        Err(err) => {
                            warn!("Failed to lock emulator mutex: {}", err);
                            return Ok(None);
                        }
                    };
                    match em.button(button.clone(), direction.clone()) {
                        Ok(()) => match direction {
                            Direction::Press => {
                                if !ctx.pressed_buttons.contains(button) {
                                    ctx.pressed_buttons.push(button.clone());
                                }
                            }
                            Direction::Release => ctx.pressed_buttons.retain(|b| b != button),
                            // Press+release in one call — nothing left held.
                            Direction::Click => {}
                        },
                        Err(err) => {
                            warn!("Failed to click button {:?} ({:?}): {}", button, direction, err);
                        }
                    }
                }
                InputToken::MoveMouse(x, y, coordinate) => {
                    let x = match ctx.resolve(x, depth).and_then(|v| v.eval_number()) {
                        Ok(v) => v.round() as i32,
                        Err(e) => {
                            warn!("Skipping mouse move: x {}", e);
                            continue;
                        }
                    };
                    let y = match ctx.resolve(y, depth).and_then(|v| v.eval_number()) {
                        Ok(v) => v.round() as i32,
                        Err(e) => {
                            warn!("Skipping mouse move: y {}", e);
                            continue;
                        }
                    };
                    let mut em = match ctx.emulator.lock() {
                        Ok(guard) => guard,
                        Err(err) => {
                            warn!("Failed to lock emulator mutex: {}", err);
                            return Ok(None);
                        }
                    };
                    let result = match coordinate {
                        Coordinate::Rel => em.move_mouse_rel(x, y),
                        Coordinate::Abs => em.move_mouse_abs(x, y),
                    };
                    if let Err(err) = result {
                        warn!("Failed to move mouse ({}, {}): {}", x, y, err);
                    }
                }
                InputToken::Scroll(amount, axis) => {
                    let amount = match ctx.resolve(amount, depth).and_then(|v| v.eval_number()) {
                        Ok(v) => v.round() as i32,
                        Err(e) => {
                            warn!("Skipping scroll: {}", e);
                            continue;
                        }
                    };
                    let mut em = match ctx.emulator.lock() {
                        Ok(guard) => guard,
                        Err(err) => {
                            warn!("Failed to lock emulator mutex: {}", err);
                            return Ok(None);
                        }
                    };
                    if let Err(err) = em.scroll(amount, axis.clone()) {
                        warn!("Failed to scroll by {}: {}", amount, err);
                    }
                }
            },
            Instruction::SetVariable(name, value) => match ctx.resolve(value, depth).and_then(|v| v.eval()) {
                Ok(result) => {
                    if let Ok(mut vars) = ctx.variables.lock() {
                        vars.insert(name.clone(), result);
                    }
                }
                Err(e) => warn!("Skipping Set Variable: {}", e),
            },
            Instruction::ChangeVariable(name, value) => match ctx.resolve(value, depth).and_then(|v| v.eval()) {
                // The delta must be numeric — text is a deliberate no-op.
                Ok(Evaluated::Text(_)) => {}
                Ok(Evaluated::Number(delta)) => {
                    if let Ok(mut vars) = ctx.variables.lock() {
                        // Non-numeric (or missing) current value coerces
                        // to 0 before adding, same leniency Scratch uses.
                        let current = vars.get(name).and_then(|e| e.as_number().ok()).unwrap_or(0.0);
                        vars.insert(name.clone(), Evaluated::Number(current + delta));
                    }
                }
                Err(e) => warn!("Skipping Change Variable: {}", e),
            },
        }
    }

    Ok(None)
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
    use crate::macros::{BlockDef, BlockPiece, Strand};

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

    fn empty_vars() -> VariableStore {
        Arc::new(Mutex::new(HashMap::new()))
    }

    fn noop_emulator() -> Arc<Mutex<dyn InputBackend>> {
        Arc::new(Mutex::new(NoopBackend))
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
            variables: vec![],
            block_defs: vec![],
        };
        let start = Instant::now();
        mac.run(noop_emulator(), None, 1.0, empty_vars());
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
            variables: vec![],
            block_defs: vec![],
        };
        let stop_flag = Arc::new(Mutex::new(true));
        let flag_clone = Arc::clone(&stop_flag);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            *flag_clone.lock().unwrap() = false;
        });
        let start = Instant::now();
        mac.run(noop_emulator(), Some(stop_flag), 1.0, empty_vars());
        assert!(start.elapsed() < Duration::from_millis(1000), "stop flag didn't stop both concurrent strands promptly");
    }

    #[test]
    fn set_variable_writes_evaluated_value() {
        let vars = empty_vars();
        run_instructions(
            vec![Instruction::SetVariable("x".to_string(), Value::number(5.0))],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(5.0)));
    }

    #[test]
    fn change_variable_adds_to_existing_numeric_value() {
        let vars = empty_vars();
        vars.lock().unwrap().insert("x".to_string(), Evaluated::Number(10.0));
        run_instructions(
            vec![Instruction::ChangeVariable("x".to_string(), Value::number(5.0))],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(15.0)));
    }

    #[test]
    fn change_variable_coerces_non_numeric_current_value_to_zero() {
        let vars = empty_vars();
        vars.lock().unwrap().insert("x".to_string(), Evaluated::Text("hello".to_string()));
        run_instructions(
            vec![Instruction::ChangeVariable("x".to_string(), Value::number(5.0))],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(5.0)));
    }

    #[test]
    fn change_variable_with_text_delta_is_a_no_op() {
        let vars = empty_vars();
        vars.lock().unwrap().insert("x".to_string(), Evaluated::Number(10.0));
        run_instructions(
            vec![Instruction::ChangeVariable("x".to_string(), Value::Text { value: "abc".to_string() })],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(10.0)));
    }

    #[test]
    fn set_variable_value_can_read_other_variables() {
        let vars = empty_vars();
        vars.lock().unwrap().insert("y".to_string(), Evaluated::Number(7.0));
        run_instructions(
            vec![Instruction::SetVariable("x".to_string(), Value::Var { name: "y".to_string() })],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(7.0)));
    }

    /// A macro with one custom "double" block (returns_value, `Return
    /// param*2`) called via `Value::Call` from a `SetVariable`'s value tree
    /// should write the doubled result — exercising the whole
    /// resolve_calls_and_params -> call_block -> run_block -> Return path.
    fn macro_with_double_block(caller_instructions: Vec<Instruction>) -> Macro {
        let block_id = "double".to_string();
        Macro {
            id: "m".into(),
            name: "Double".into(),
            description: "".into(),
            strands: vec![
                Strand {
                    id: "caller".into(),
                    x: 0,
                    y: 0,
                    instructions: {
                        let mut ins = vec![Instruction::WhenRan];
                        ins.extend(caller_instructions);
                        ins
                    },
                },
                Strand {
                    id: "double_body".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::BlockHeader(block_id.clone()),
                        Instruction::Return(Value::Op {
                            op: crate::input::value::Op::Mul,
                            args: vec![Value::Param { name: "n".into() }, Value::number(2.0)],
                            saved: Box::new(Value::number(0.0)),
                        }),
                    ],
                },
            ],
            recording_target: None,
            speed_multiplier: 1.0,
            floating_values: vec![],
            variables: vec![],
            block_defs: vec![BlockDef {
                id: block_id,
                pieces: vec![BlockPiece::Label { id: "p1".into(), text: "double".into() }, BlockPiece::Input { id: "p2".into(), name: "n".into() }],
                returns_value: true,
            }],
        }
    }

    #[test]
    fn value_call_to_reporter_block_resolves_and_runs() {
        let vars = empty_vars();
        let mac = macro_with_double_block(vec![Instruction::SetVariable(
            "x".to_string(),
            Value::Call { block_id: "double".to_string(), args: vec![Value::number(21.0)], saved: Box::new(Value::number(0.0)) },
        )]);
        mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
        assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(42.0)));
    }

    /// A `returns_value` block whose body never hits `Return` should leave
    /// the caller's `SetVariable` un-applied (the whole resolve chain errs,
    /// same as any other unresolvable value — logged and skipped) rather
    /// than panicking or silently defaulting to some value.
    #[test]
    fn value_call_to_block_without_return_leaves_variable_unset() {
        let vars = empty_vars();
        let block_id = "empty".to_string();
        let mac = Macro {
            id: "m".into(),
            name: "Empty".into(),
            description: "".into(),
            strands: vec![
                Strand {
                    id: "caller".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::WhenRan,
                        Instruction::SetVariable(
                            "x".to_string(),
                            Value::Call { block_id: block_id.clone(), args: vec![], saved: Box::new(Value::number(0.0)) },
                        ),
                    ],
                },
                Strand { id: "empty_body".into(), x: 0, y: 0, instructions: vec![Instruction::BlockHeader(block_id.clone())] },
            ],
            recording_target: None,
            speed_multiplier: 1.0,
            floating_values: vec![],
            variables: vec![],
            block_defs: vec![BlockDef { id: block_id, pieces: vec![], returns_value: true }],
        };
        mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
        assert_eq!(vars.lock().unwrap().get("x"), None);
    }

    /// A `CallBlock` (command position) with real `Wait`s inside its body
    /// should actually take real time, proving the callee's instructions
    /// genuinely execute inline rather than being skipped/no-op'd.
    #[test]
    fn call_block_runs_real_instructions_including_wait() {
        let block_id = "waiter".to_string();
        let mac = Macro {
            id: "m".into(),
            name: "Waiter".into(),
            description: "".into(),
            strands: vec![
                Strand {
                    id: "caller".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::WhenRan,
                        Instruction::CallBlock { block_id: block_id.clone(), args: vec![] },
                    ],
                },
                Strand {
                    id: "waiter_body".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![Instruction::BlockHeader(block_id.clone()), Instruction::Wait(Value::number(150.0))],
                },
            ],
            recording_target: None,
            speed_multiplier: 1.0,
            floating_values: vec![],
            variables: vec![],
            block_defs: vec![BlockDef { id: block_id, pieces: vec![], returns_value: false }],
        };
        let start = Instant::now();
        mac.run(noop_emulator(), None, 1.0, empty_vars());
        assert!(start.elapsed() >= Duration::from_millis(140), "CallBlock's nested Wait didn't actually take real time");
    }

    /// A block that calls itself should error past `MAX_CALL_DEPTH` instead
    /// of overflowing the stack.
    #[test]
    fn self_recursive_reporter_block_errs_past_max_depth() {
        let vars = empty_vars();
        let block_id = "loop".to_string();
        let mac = Macro {
            id: "m".into(),
            name: "Loop".into(),
            description: "".into(),
            strands: vec![
                Strand {
                    id: "caller".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::WhenRan,
                        Instruction::SetVariable(
                            "x".to_string(),
                            Value::Call { block_id: block_id.clone(), args: vec![], saved: Box::new(Value::number(0.0)) },
                        ),
                    ],
                },
                Strand {
                    id: "loop_body".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::BlockHeader(block_id.clone()),
                        Instruction::Return(Value::Call { block_id: block_id.clone(), args: vec![], saved: Box::new(Value::number(0.0)) }),
                    ],
                },
            ],
            recording_target: None,
            speed_multiplier: 1.0,
            floating_values: vec![],
            variables: vec![],
            block_defs: vec![BlockDef { id: block_id, pieces: vec![], returns_value: true }],
        };
        // Should return promptly (erroring out at MAX_CALL_DEPTH) rather than
        // hanging or crashing the test process via a stack overflow.
        mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
        assert_eq!(vars.lock().unwrap().get("x"), None);
    }

    /// One reporter block ("triple") calling another ("double") in its own
    /// `Return` expression should compose correctly — proves multi-level
    /// `Value::Call` resolution genuinely nests rather than only working one
    /// level deep.
    #[test]
    fn reporter_block_composes_with_another_reporter_block() {
        let vars = empty_vars();
        let double_id = "double".to_string();
        let triple_id = "triple".to_string();
        let mac = Macro {
            id: "m".into(),
            name: "Compose".into(),
            description: "".into(),
            strands: vec![
                Strand {
                    id: "caller".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::WhenRan,
                        Instruction::SetVariable(
                            "x".to_string(),
                            Value::Call {
                                block_id: triple_id.clone(),
                                args: vec![Value::number(2.0)],
                                saved: Box::new(Value::number(0.0)),
                            },
                        ),
                    ],
                },
                Strand {
                    id: "double_body".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::BlockHeader(double_id.clone()),
                        Instruction::Return(Value::Op {
                            op: crate::input::value::Op::Mul,
                            args: vec![Value::Param { name: "n".into() }, Value::number(2.0)],
                            saved: Box::new(Value::number(0.0)),
                        }),
                    ],
                },
                Strand {
                    id: "triple_body".into(),
                    x: 0,
                    y: 0,
                    // triple(n) = double(n) + n  =>  triple(2) = 4 + 2 = 6
                    instructions: vec![
                        Instruction::BlockHeader(triple_id.clone()),
                        Instruction::Return(Value::Op {
                            op: crate::input::value::Op::Add,
                            args: vec![
                                Value::Call {
                                    block_id: double_id.clone(),
                                    args: vec![Value::Param { name: "n".into() }],
                                    saved: Box::new(Value::number(0.0)),
                                },
                                Value::Param { name: "n".into() },
                            ],
                            saved: Box::new(Value::number(0.0)),
                        }),
                    ],
                },
            ],
            recording_target: None,
            speed_multiplier: 1.0,
            floating_values: vec![],
            variables: vec![],
            block_defs: vec![
                BlockDef {
                    id: double_id,
                    pieces: vec![BlockPiece::Input { id: "p1".into(), name: "n".into() }],
                    returns_value: true,
                },
                BlockDef {
                    id: triple_id,
                    pieces: vec![BlockPiece::Input { id: "p1".into(), name: "n".into() }],
                    returns_value: true,
                },
            ],
        };
        mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
        assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(6.0)));
    }
}
