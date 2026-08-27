use crate::input::types::{Axis, Coordinate, Direction};

pub fn direction_to_index(direction: &Direction) -> usize {
    match direction {
        Direction::Click => 0,
        Direction::Press => 1,
        Direction::Release => 2,
    }
}

pub fn index_to_direction(index: usize) -> Direction {
    match index {
        0 => Direction::Click,
        1 => Direction::Press,
        2 => Direction::Release,
        _ => Direction::Click,
    }
}

pub fn coordinate_to_index(coordinate: &Coordinate) -> usize {
    match coordinate {
        Coordinate::Abs => 0,
        Coordinate::Rel => 1,
    }
}

pub fn index_to_coordinate(index: usize) -> Coordinate {
    match index {
        0 => Coordinate::Abs,
        1 => Coordinate::Rel,
        _ => Coordinate::Abs,
    }
}

pub fn axis_to_index(axis: &Axis) -> usize {
    match axis {
        Axis::Vertical => 0,
        Axis::Horizontal => 1,
    }
}

pub fn index_to_axis(index: usize) -> Axis {
    match index {
        0 => Axis::Vertical,
        1 => Axis::Horizontal,
        _ => Axis::Vertical,
    }
}

pub fn get_direction_names() -> &'static [&'static str] {
    &["Click", "Press", "Release"]
}

pub fn get_coordinate_names() -> &'static [&'static str] {
    &["Absolute", "Relative"]
}

pub fn get_axis_names() -> &'static [&'static str] {
    &["Vertical", "Horizontal"]
}
