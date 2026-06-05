use enigo::{Axis, Button, Coordinate, Direction, Key as EnigoKey};
use rdev::{EventType, Key as RdevKey, simulate};
use tracing::warn;

use crate::macros::key_mapping::{char_to_rdev, enigo_button_to_rdev, enigo_key_to_rdev};

pub struct UinputEmulator;

impl UinputEmulator {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self)
    }

    fn sim(event: &EventType) {
        if let Err(e) = simulate(event) {
            warn!("rdev simulate error: {:?}", e);
        }
    }

    fn press_key(key: RdevKey) {
        Self::sim(&EventType::KeyPress(key));
    }

    fn release_key(key: RdevKey) {
        Self::sim(&EventType::KeyRelease(key));
    }

    pub fn key(&mut self, key: EnigoKey, dir: Direction) -> Result<(), String> {
        let (rdev_key, needs_shift) = enigo_key_to_rdev(&key)
            .ok_or_else(|| format!("no rdev mapping for {:?}", key))?;
        match dir {
            Direction::Press => {
                if needs_shift {
                    Self::press_key(RdevKey::ShiftLeft);
                }
                Self::press_key(rdev_key);
            }
            Direction::Release => {
                Self::release_key(rdev_key);
                if needs_shift {
                    Self::release_key(RdevKey::ShiftLeft);
                }
            }
            Direction::Click => {
                if needs_shift {
                    Self::press_key(RdevKey::ShiftLeft);
                }
                Self::press_key(rdev_key);
                Self::release_key(rdev_key);
                if needs_shift {
                    Self::release_key(RdevKey::ShiftLeft);
                }
            }
        }
        Ok(())
    }

    pub fn raw(&mut self, keycode: u16, dir: Direction) -> Result<(), String> {
        let key = RdevKey::Unknown(keycode as u32);
        match dir {
            Direction::Press => Self::press_key(key),
            Direction::Release => Self::release_key(key),
            Direction::Click => {
                Self::press_key(key);
                Self::release_key(key);
            }
        }
        Ok(())
    }

    pub fn button(&mut self, button: Button, dir: Direction) -> Result<(), String> {
        // Scroll "buttons" become Wheel events
        let wheel: Option<(i64, i64)> = match button {
            Button::ScrollUp => Some((0, 1)),
            Button::ScrollDown => Some((0, -1)),
            Button::ScrollLeft => Some((-1, 0)),
            Button::ScrollRight => Some((1, 0)),
            _ => None,
        };
        if let Some((dx, dy)) = wheel {
            Self::sim(&EventType::Wheel { delta_x: dx, delta_y: dy });
            return Ok(());
        }

        let rdev_btn = enigo_button_to_rdev(button)
            .ok_or_else(|| format!("no rdev mapping for {:?}", button))?;
        match dir {
            Direction::Press => Self::sim(&EventType::ButtonPress(rdev_btn)),
            Direction::Release => Self::sim(&EventType::ButtonRelease(rdev_btn)),
            Direction::Click => {
                Self::sim(&EventType::ButtonPress(rdev_btn));
                Self::sim(&EventType::ButtonRelease(rdev_btn));
            }
        }
        Ok(())
    }

    pub fn move_mouse(&mut self, x: i32, y: i32, coord: Coordinate) -> Result<(), String> {
        let (abs_x, abs_y) = match coord {
            Coordinate::Rel => {
                let (cur_x, cur_y) = crate::recording::get_last_mouse_pos()
                    .map(|(px, py)| (px as i32, py as i32))
                    .unwrap_or_else(|| {
                        warn!("mouse position unknown; defaulting to (0, 0) for relative move");
                        (0, 0)
                    });
                (cur_x + x, cur_y + y)
            }
            Coordinate::Abs => (x, y),
        };
        Self::sim(&EventType::MouseMove { x: abs_x as f64, y: abs_y as f64 });
        crate::recording::set_last_mouse_pos(abs_x as f64, abs_y as f64);
        Ok(())
    }

    pub fn scroll(&mut self, amount: i32, axis: Axis) -> Result<(), String> {
        let (dx, dy) = match axis {
            Axis::Vertical => (0, amount as i64),
            Axis::Horizontal => (amount as i64, 0),
        };
        Self::sim(&EventType::Wheel { delta_x: dx, delta_y: dy });
        Ok(())
    }

    pub fn text(&mut self, s: &str) -> Result<(), String> {
        for c in s.chars() {
            match char_to_rdev(c) {
                Some((key, needs_shift)) => {
                    if needs_shift {
                        Self::press_key(RdevKey::ShiftLeft);
                    }
                    Self::press_key(key);
                    Self::release_key(key);
                    if needs_shift {
                        Self::release_key(RdevKey::ShiftLeft);
                    }
                }
                None => warn!("no rdev mapping for char {:?}, skipping", c),
            }
        }
        Ok(())
    }
}
