use std::path::Path;

use bevy::prelude::*;
use ferrosa_protocol::clientbound::game::{GameState, VehicleKind};
use rodio::{Source, buffer::SamplesBuffer};

use super::{
    Level,
    netgame::NetGame,
    smoothing::Smoothing,
    sounds::{Ears, Emitter, decode},
    vehicles::NetVehicle,
    voice::line_of_sight,
};
use crate::{audio::Speakers, menu::Screen};

const SLOTS: usize = 8;
const RANGE: f32 = 256.0;
const ENGINE_RADIUS: f32 = 10.0;
const EASE: f32 = 0.125;
const SKIDDING: f32 = 0.0625;
const IDLE_RPM: i32 = 0x63;
const IDLE_PITCH: f32 = 0.298_828_13;

#[derive(Resource)]
struct EngineClips {
    engine: SamplesBuffer,
    tires: SamplesBuffer,
}

struct Slot {
    vehicle: usize,
    engine: Option<Emitter>,
    tires: Option<Emitter>,
    engine_volume: f32,
    tire_volume: f32,
}

#[derive(Resource, Default)]
struct Engines(Vec<Slot>);

pub struct EnginesPlugin;

impl Plugin for EnginesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Engines>()
            .add_systems(Startup, load_engine_clips)
            .add_systems(Update, hear_engines.after(Smoothing::View).run_if(in_state(Screen::InGame)).run_if(resource_exists::<NetGame>).run_if(resource_exists::<EngineClips>))
            .add_systems(OnExit(Screen::InGame), |mut engines: ResMut<Engines>| engines.0.clear());
    }
}

fn load_engine_clips(mut commands: Commands) {
    let file = |name: &str| decode(&Path::new("data/sound").join(name));
    let (Some(engine), Some(tires)) = (file("4cylinder.wav"), file("tiresqueal.wav")) else { return };

    commands.insert_resource(EngineClips { engine, tires });
}

fn heard(kind: VehicleKind) -> bool {
    !matches!(kind, VehicleKind::Helicopter | VehicleKind::NoClip | VehicleKind::Train)
}

fn start(speakers: &Speakers, clip: &SamplesBuffer, at: Vec3, radius: f32, ears: &Ears, volume: f32) -> Option<Emitter> {
    let mixer = speakers.0.as_ref()?;
    let emitter = Emitter::new(mixer, at, radius, ears);

    emitter.player().set_volume(volume * ears.volume);
    emitter.player().append(clip.clone().repeat_infinite());

    Some(emitter)
}

fn hear_engines(
    net: Res<NetGame>,
    clips: Res<EngineClips>,
    speakers: Res<Speakers>,
    ears: Res<Ears>,
    level: Option<Res<Level>>,
    mut engines: ResMut<Engines>,
    vehicles: Query<(&NetVehicle, &Transform)>,
) {
    if net.game_state == GameState::Restarting {
        engines.0.clear();
        return;
    }

    let listener = (ears.left + ears.right) * 0.5;
    let place = |v: usize| vehicles.iter().find(|(nv, _)| nv.index == v).map(|(_, t)| *t);
    let distance = |v: usize| place(v).map_or(f32::MAX, |t| t.translation.distance(listener));
    let sight = |from: Vec3| level.as_ref().map_or(1.0, |l| line_of_sight(l, from, listener));
    let running = |v: usize| net.vehicles.get(v).is_some_and(|v| v.active && heard(v.kind) && v.engine_rpm as i32 > 0);

    for (nv, _) in &vehicles {
        let i = nv.index;

        if !running(i) || engines.0.iter().any(|s| s.vehicle == i) {
            continue;
        }

        let fresh = Slot { vehicle: i, engine: None, tires: None, engine_volume: 0.0, tire_volume: 0.0 };

        if engines.0.len() < SLOTS {
            engines.0.push(fresh);
            continue;
        }

        let here = distance(i);
        let farther = engines.0.iter().enumerate().map(|(j, s)| (j, distance(s.vehicle))).filter(|&(_, d)| d > here).min_by(|a, b| a.1.total_cmp(&b.1));

        if let Some((j, _)) = farther {
            engines.0[j] = fresh;
        }
    }

    for s in &mut engines.0 {
        let Some(at) = place(s.vehicle) else {
            (s.engine, s.tires) = (None, None);
            continue;
        };

        if !running(s.vehicle) || at.translation.distance(listener) >= RANGE {
            (s.engine, s.tires) = (None, None);
            continue;
        }

        let v = &net.vehicles[s.vehicle];
        let (up, back) = (at.rotation * Vec3::Y, at.rotation * Vec3::Z);
        let front = at.translation - back * 2.5 - up * 0.25;
        let rpm = v.engine_rpm as i32;

        if s.engine.is_none() {
            s.engine = start(&speakers, &clips.engine, front, ENGINE_RADIUS, &ears, 0.5);
        }

        if let Some(engine) = s.engine.as_mut() {
            let pitch = if rpm > IDLE_RPM { rpm as f32 * 0.000_488_281_25 + 0.25 } else { IDLE_PITCH };
            let volume = (rpm as f32 * 0.015625).min(0.5) * 0.5;

            s.engine_volume += (sight(front) * volume - s.engine_volume) * EASE;

            engine.place(front, ENGINE_RADIUS, &ears);
            engine.player().set_speed(pitch);
            engine.player().set_volume(s.engine_volume * ears.volume);
        }

        let rear = at.translation + back * 0.125 - up * 0.25;
        let skid: f32 = v.wheels.iter().map(|w| w[2]).sum();

        if s.tires.is_none() && skid > SKIDDING {
            let first = skid.min(0.25);
            s.tires = start(&speakers, &clips.tires, rear, 10.0, &ears, first);
            s.tire_volume = first;
        }

        if let Some(tires) = s.tires.as_mut() {
            s.tire_volume += (sight(rear) * skid.min(1.0) - s.tire_volume) * EASE;

            tires.place(rear, 10.0, &ears);
            tires.player().set_volume(s.tire_volume * ears.volume);

            if s.tire_volume < SKIDDING {
                s.tires = None;
            }
        }
    }

    engines.0.retain(|s| s.engine.is_some() || s.tires.is_some());
}
