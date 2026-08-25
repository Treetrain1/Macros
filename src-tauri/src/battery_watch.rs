//! Background service that fires `WhenBatteryDischargedTo`/
//! `WhenBatteryChargedTo` strands on their own, independent of Run/Loop —
//! see `runner::run_with_offset`'s comment on why those strands are excluded
//! from a normal Run. This is the thing that actually watches the battery:
//! it polls every macro's strands on a timer for the lifetime of the app,
//! and fires a strand's body directly (skipping its header) the moment the
//! battery crosses that strand's threshold.
//!
//! Each watched strand is edge-triggered with simple hysteresis rather than
//! level-triggered: once fired, a strand won't fire again until the battery
//! recovers back past its threshold (charges back up for a discharge
//! trigger, drains back down for a charge trigger) and crosses it again —
//! otherwise it would refire on every poll tick for as long as the battery
//! sits at that level. A strand already satisfying its condition the first
//! time this watcher ever sees it (e.g. right at app startup) still fires
//! immediately — there's no "must have just crossed" requirement on the very
//! first observation.

use crate::state::{build_state_dto, SharedState};
use macros_core::input::value::Value;
use macros_core::macros::backend::InputBackend;
use macros_core::macros::runner::{run_instructions, VariableStore};
use macros_core::macros::{run_registry, Instruction};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Runtime};
use tracing::warn;

/// How often the watcher re-reads the battery and re-checks every macro's
/// strands. Battery level changes over minutes, not seconds, so this is
/// deliberately coarse.
const POLL_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Arm {
    /// Ready to fire the next time its condition becomes true.
    Armed,
    /// Already fired for the current crossing; waiting for the battery to
    /// recover back past the threshold before it can arm again.
    Disarmed,
}

/// Decides what one tick does for one strand, given whether its condition
/// currently holds and its arm state going in. Pure (no I/O, no locking) so
/// the fire-once/wait-for-recovery behavior is unit-testable without a real
/// battery reading, a running app, or a spawned thread. Returns `(should
/// fire this tick, arm state for the next tick)`.
fn tick_decision(armed: Arm, holds: bool) -> (bool, Arm) {
    match (armed, holds) {
        (Arm::Armed, true) => (true, Arm::Disarmed),
        (Arm::Disarmed, false) => (false, Arm::Armed),
        (armed, _) => (false, armed),
    }
}

/// Spawns the watcher thread. Runs for the lifetime of the app; there's no
/// handle to stop it since it only ever does anything when a macro actually
/// declares one of these blocks.
pub(crate) fn start<R: Runtime>(shared_state: SharedState, app: AppHandle<R>) {
    let _ = std::thread::Builder::new().name("battery-watch".into()).spawn(move || run(shared_state, app));
}

fn run<R: Runtime>(shared_state: SharedState, app: AppHandle<R>) {
    // (macro_id, strand_id) -> arm state. Rebuilt fresh each tick from
    // whatever strands currently exist, carrying over prior arm state by key
    // — so a deleted strand/macro just quietly drops out instead of leaking.
    let mut arm_state: HashMap<(String, String), Arm> = HashMap::new();

    loop {
        std::thread::sleep(POLL_INTERVAL);

        let level = match macros_core::battery::percentage() {
            Ok(l) => l,
            // No battery on this machine (or the platform API is
            // unavailable) — nothing to watch, just keep idling; a desktop
            // shouldn't burn a thread hammering a battery API it doesn't have.
            Err(_) => continue,
        };

        let (macros, emulator, speed_multiplier) = {
            let Ok(s) = shared_state.lock() else { continue };
            let Some(emulator) = s.emulator.as_ref().map(Arc::clone) else { continue };
            (s.macros_list.clone(), emulator, s.global_speed_multiplier)
        };

        let mut next_arm_state = HashMap::with_capacity(arm_state.len());
        for mac in &macros {
            for strand in &mac.strands {
                let (threshold, satisfied): (&Value, fn(f64, f64) -> bool) = match strand.instructions.first() {
                    Some(Instruction::WhenBatteryDischargedTo(v)) => (v, |level, threshold| level <= threshold),
                    Some(Instruction::WhenBatteryChargedTo(v)) => (v, |level, threshold| level >= threshold),
                    _ => continue,
                };
                // Only resolves plain numbers/operators — a threshold that
                // reads a macro variable has no live value here (this isn't
                // a real run) and just defaults that read to 0, same as
                // `Value::resolve_vars` does for any other missing name.
                let Ok(threshold) = threshold.resolve_vars(&HashMap::new()).eval_number() else { continue };

                let key = (mac.id.clone(), strand.id.clone());
                let holds = satisfied(level, threshold);
                let armed = arm_state.get(&key).copied().unwrap_or(Arm::Armed);
                let (should_fire, next) = tick_decision(armed, holds);

                if should_fire {
                    fire(mac.id.clone(), strand.instructions[1..].to_vec(), Arc::clone(&emulator), speed_multiplier, &mac.variables, Arc::clone(&shared_state), app.clone());
                }
                next_arm_state.insert(key, next);
            }
        }
        arm_state = next_arm_state;
    }
}

/// Runs one battery-triggered strand's body on its own thread — mirrors
/// `macros_thread::into_single_run_task`, but for a strand fired by this
/// watcher rather than the Run button. Variables start fresh from the
/// macro's own declared defaults (there's no "live" store for a macro that
/// isn't open/running) and are written back to that macro's saved variables
/// once the fired strand finishes, same as a normal run does.
fn fire<R: Runtime>(
    macro_id: String,
    instructions: Vec<Instruction>,
    emulator: Arc<Mutex<dyn InputBackend>>,
    speed_multiplier: f64,
    variable_defs: &[macros_core::macros::VariableDef],
    shared_state: SharedState,
    app: AppHandle<R>,
) {
    let variables: VariableStore = Arc::new(Mutex::new(variable_defs.iter().map(|v| (v.name.clone(), v.value.clone())).collect()));
    std::thread::spawn(move || {
        let run_flag = run_registry::begin_run();
        run_instructions(instructions, emulator, Some(Arc::clone(&run_flag)), speed_multiplier, Arc::clone(&variables));
        run_registry::end_run(&run_flag);
        persist_fired_variables(&shared_state, &app, &macro_id, &variables);
    });
}

/// Like `macros_thread::persist_variables`, but looks the macro up by id
/// across the whole macro list instead of only the currently-selected one —
/// a battery-fired macro is very often not the one open in the editor.
fn persist_fired_variables<R: Runtime>(shared_state: &SharedState, app: &AppHandle<R>, macro_id: &str, variables: &VariableStore) {
    let (mac_to_save, dto) = {
        let Ok(mut s) = shared_state.lock() else { return };
        let Some(mac) = s.macros_list.iter_mut().find(|m| m.id == macro_id) else { return };
        if let Ok(values) = variables.lock() {
            mac.sync_variables_from(&values);
        }
        let mac_to_save = mac.clone();
        if s.current_macro.as_ref().is_some_and(|c| c.id == macro_id) {
            s.current_macro = Some(mac_to_save.clone());
        }
        (mac_to_save, build_state_dto(&s))
    };
    if let Err(e) = mac_to_save.save() {
        warn!("Failed to persist battery-fired macro's variables: {e}");
    }
    let _ = app.emit("state-updated", dto);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fires_once_then_waits_for_recovery_before_rearming() {
        // First observation, condition already true (e.g. app just started
        // with the battery already past the threshold): fires immediately.
        let (fired, state) = tick_decision(Arm::Armed, true);
        assert!(fired);
        assert_eq!(state, Arm::Disarmed);

        // Stays disarmed — no refire — while the condition keeps holding on
        // later ticks.
        let (fired, state) = tick_decision(state, true);
        assert!(!fired);
        assert_eq!(state, Arm::Disarmed);

        // Recovers past the threshold: re-arms, but that's not itself a fire.
        let (fired, state) = tick_decision(state, false);
        assert!(!fired);
        assert_eq!(state, Arm::Armed);

        // Crosses again: fires again.
        let (fired, state) = tick_decision(state, true);
        assert!(fired);
        assert_eq!(state, Arm::Disarmed);
    }

    #[test]
    fn never_fires_while_condition_stays_false() {
        let (fired, state) = tick_decision(Arm::Armed, false);
        assert!(!fired);
        assert_eq!(state, Arm::Armed);

        let (fired, state) = tick_decision(state, false);
        assert!(!fired);
        assert_eq!(state, Arm::Armed);
    }
}
