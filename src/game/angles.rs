use std::f32::consts::{PI, TAU};

pub fn angle_difference(a: f32, b: f32) -> f32 {
    (a - b + PI).rem_euclid(TAU) - PI
}

pub fn wrap_angle(a: f32) -> f32 {
    angle_difference(a, 0.0)
}
