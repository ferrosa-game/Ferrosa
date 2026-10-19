use bevy::prelude::*;
use ferrosa_protocol::{GameMode, clientbound::initial_sync::InitialSync};

#[derive(Resource, Clone, Debug)]
pub struct ServerSync {
    pub round: u32,
    pub game_type: GameMode,
    pub weekly: bool,
    pub weekday: u32,
    pub map: String,
    pub sun_angle: f32,
}

fn angle16(v: u16) -> f32 {
    let sign = 0x8000u32;
    let a = ((((sign - 1) & v as u32) as f32) as f64 / ((sign as f32) as f64 / std::f64::consts::TAU)) as f32;
    if v as u32 & sign == sign { -a } else { a }
}

impl From<InitialSync> for ServerSync {
    fn from(s: InitialSync) -> Self {
        Self {
            round: s.round_number,
            game_type: s.gamemode,
            weekly: s.weekly_enabled,
            weekday: s.weekday as u32,
            map: s.map_name,
            sun_angle: angle16(s.sun_angle),
        }
    }
}
