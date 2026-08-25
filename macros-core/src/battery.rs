//! Cross-platform battery percentage reads, backing `Op::BatteryPercentage`
//! and the `WhenBatteryDischargedTo`/`WhenBatteryChargedTo` instructions.

use starship_battery::units::ratio::percent;

/// Charge (0-100) of the system's first detected battery. Errs if the
/// platform's battery API is unavailable or no battery is present (e.g. a
/// desktop with no battery at all).
pub fn percentage() -> Result<f64, String> {
    let manager = starship_battery::Manager::new().map_err(|e| format!("battery info unavailable: {e}"))?;
    let battery = manager
        .batteries()
        .map_err(|e| format!("failed to enumerate batteries: {e}"))?
        .next()
        .ok_or_else(|| "no battery detected".to_string())?
        .map_err(|e| format!("failed to read battery: {e}"))?;
    Ok(battery.state_of_charge().get::<percent>() as f64)
}
