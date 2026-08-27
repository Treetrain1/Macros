//! Background service that fires `WhenTime` strands on their own, independent
//! of Run/Loop — see `battery_watch`'s module doc for why (same mechanism,
//! same reasoning, just a time condition instead of a battery one). Polls
//! every macro's strands on a timer and fires a strand's body directly
//! (skipping its header) the moment local time matches its schedule.
//!
//! Only the currently selected macro's strands are watched, plus any macro
//! whose `MacroSettings::always_listen` is set — see `battery_watch`'s module
//! doc for the same scoping rule.
//!
//! Unlike the battery watcher's threshold-crossing hysteresis, a `WhenTime`
//! schedule dedups by calendar date: each schedule fires at most once per
//! date whose weekday/day-of-month/month+day satisfies it (which, given how
//! `TimeSchedule::matches` is defined, is exactly once per day/week/month/
//! year respectively) — tracked as "the last date this schedule fired on".
//! Time only moves forward, so unlike a battery level there's nothing to
//! "recover past" before it can fire again; the date rolling over is enough.

use crate::scheduled_run;
use crate::state::SharedState;
use chrono::{Local, NaiveDate};
use blockwork_core::macros::InstructionKind;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Runtime};

/// How often the watcher re-checks the clock against every macro's
/// schedules. A schedule fires up to this long after its target minute
/// actually starts (worst case: the minute rolls over right after a tick),
/// so this is kept tight — unlike the battery watcher's 5s poll, reading the
/// local clock and walking the macro list is cheap enough that there's no
/// real cost to checking every second.
const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Spawns the watcher thread. Runs for the lifetime of the app; there's no
/// handle to stop it since it only ever does anything when a macro actually
/// declares a `WhenTime` block.
pub(crate) fn start<R: Runtime>(shared_state: SharedState, app: AppHandle<R>) {
    let _ = std::thread::Builder::new().name("time-watch".into()).spawn(move || run(shared_state, app));
}

fn run<R: Runtime>(shared_state: SharedState, app: AppHandle<R>) {
    // (macro_id, strand_id) -> the date this schedule last fired on.
    // Rebuilt fresh each tick from whatever strands currently exist,
    // carrying over the prior value by key — so a deleted strand/macro just
    // quietly drops out instead of leaking.
    let mut last_fired: HashMap<(String, String), NaiveDate> = HashMap::new();

    loop {
        std::thread::sleep(POLL_INTERVAL);

        let now = Local::now();
        let today = now.date_naive();

        let (macros, emulator, speed_multiplier, selected_id) = {
            let Ok(s) = shared_state.lock() else { continue };
            let Some(emulator) = s.emulator.as_ref().map(Arc::clone) else { continue };
            let selected_id = s.macro_selected.and_then(|i| s.macros_list.get(i)).map(|m| m.id.clone());
            (s.macros_list.clone(), emulator, s.global_speed_multiplier, selected_id)
        };

        let mut next_last_fired = HashMap::with_capacity(last_fired.len());
        for mac in &macros {
            if !mac.settings.always_listen && selected_id.as_deref() != Some(mac.id.as_str()) {
                continue;
            }
            for strand in &mac.strands {
                let Some(InstructionKind::WhenTime(schedule)) = strand.instructions.first().map(|i| &i.kind) else { continue };

                let key = (mac.id.clone(), strand.id.clone());
                let already_fired_today = last_fired.get(&key) == Some(&today);

                if !already_fired_today && schedule.matches(&now) {
                    scheduled_run::fire(mac.id.clone(), strand.instructions[1..].to_vec(), Arc::clone(&emulator), speed_multiplier, &mac.variables, Arc::clone(&shared_state), app.clone());
                    next_last_fired.insert(key, today);
                } else if already_fired_today {
                    next_last_fired.insert(key, today);
                }
            }
        }
        last_fired = next_last_fired;
    }
}
