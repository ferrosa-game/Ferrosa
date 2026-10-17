use std::{num::NonZero, path::Path};

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
const TYRE_RADIUS: f32 = 10.0;
const TRAIN_RADIUS: f32 = 32.0;
const RATE: u32 = 48000;
const ROTOR_LENGTH: usize = 0x10000;
const ROTOR_WARM_UP: usize = 0x4000;
const TRAIN_ROTOR: usize = 3;
const EASE: f32 = 0.125;
const SKIDDING: f32 = 0.0625;
const IDLE_RPM: i32 = 0x63;
const IDLE_PITCH: f32 = 0.298_828_13;

#[derive(Resource)]
struct EngineClips {
    engine: SamplesBuffer,
    tyres: SamplesBuffer,
    train: SamplesBuffer,
}

struct Slot {
    vehicle: usize,
    engine: Option<Emitter>,
    tyres: Option<Emitter>,
    engine_volume: f32,
    tyre_volume: f32,
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

fn hash_noise(n: i32) -> f32 {
    let n = n ^ (n << 13);
    let t = n.wrapping_mul(n).wrapping_mul(0x3d73).wrapping_add(0xc0ae5);

    1.0 - ((t.wrapping_mul(t).wrapping_add(0x5208dd0d)) & 0x7fff_ffff) as f32 * 9.313_225_7e-10
}

fn rotor(loop_index: usize) -> Vec<f32> {
    let (noisy, fifth) = (loop_index == 1, loop_index == 2);
    let step = if fifth { 0.007_812_5f32 } else { 0.003_906_25 };
    let mut buf = vec![0i16; ROTOR_LENGTH];
    let mut phase = 0.0f32;

    for s in buf.iter_mut() {
        let saw = ((phase as f64 - phase.floor() as f64).abs() * 16384.0 - 8192.0) as i32 as i16;

        *s = if fifth {
            let p = phase * 1.5;
            (saw as f64 + (p as f64 - p.floor() as f64).abs() * 8192.0 - 4096.0) as i32 as i16
        } else {
            saw
        };

        phase += step;
    }

    let resonance = if noisy { 0.125f32 } else if fifth { 1.0 } else { 4.0 };
    let mut y = [0.0f32; 4];
    let mut filter = |buf: &mut [i16], i: usize, store: bool| {
        let k = if noisy { hash_noise((i as f32 * 0.0625 * 0.0625) as i32) * 0.5 + 1.0 } else { resonance };
        let x = buf[i] as f32 / 32768.0 - y[3] * k;

        y[0] += (x - y[0]) * 0.025;
        y[1] += (y[0] - y[1]) * 0.025;
        y[2] += (y[1] - y[2]) * 0.025;

        let t = (y[2] - y[3]) * 0.025 + y[3];
        y[3] = t - t * t * t / 6.0;

        if store {
            buf[i] = (y[3] * 65536.0) as i32 as i16;
        }
    };

    for i in 0..ROTOR_LENGTH {
        filter(&mut buf, i, i >= ROTOR_WARM_UP);
    }

    for i in 0..ROTOR_WARM_UP {
        filter(&mut buf, i, true);
    }

    let gain = if fifth { 2 } else { 3 };

    buf.into_iter().map(|s| s.wrapping_mul(gain) as f32 / 32768.0).collect()
}

fn load_engine_clips(mut commands: Commands) {
    let file = |name: &str| decode(&Path::new("data/sound").join(name));
    let (Some(engine), Some(tyres)) = (file("4cylinder.wav"), file("tiresqueal.wav")) else { return };
    let train = SamplesBuffer::new(NonZero::<u16>::MIN, NonZero::new(RATE).expect("a rate"), rotor(TRAIN_ROTOR));

    commands.insert_resource(EngineClips { engine, tyres, train });
}

fn heard(kind: VehicleKind) -> bool {
    !matches!(kind, VehicleKind::Helicopter | VehicleKind::NoClip)
}

fn start(speakers: &Speakers, clip: &SamplesBuffer, at: Vec3, radius: f32, ears: &Ears, volume: f32) -> Option<Emitter> {
    let mixer = speakers.0.as_ref()?;
    let emitter = Emitter::new(mixer, at, radius, ears);

    emitter.player().set_volume(volume * ears.volume);
    emitter.player().append(clip.clone().repeat_infinite());

    Some(emitter)
}

#[allow(clippy::too_many_arguments)]
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

        let fresh = Slot { vehicle: i, engine: None, tyres: None, engine_volume: 0.0, tyre_volume: 0.0 };

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
            (s.engine, s.tyres) = (None, None);
            continue;
        };

        if !running(s.vehicle) || at.translation.distance(listener) >= RANGE {
            (s.engine, s.tyres) = (None, None);
            continue;
        }

        let v = &net.vehicles[s.vehicle];
        let (up, back) = (at.rotation * Vec3::Y, at.rotation * Vec3::Z);
        let train = v.kind == VehicleKind::Train;
        let front = at.translation - back * 2.5 - up * 0.25;
        let rpm = v.engine_rpm as i32;

        if s.engine.is_none() {
            s.engine = if train { start(&speakers, &clips.train, front, TRAIN_RADIUS, &ears, 0.125) } else { start(&speakers, &clips.engine, front, ENGINE_RADIUS, &ears, 0.5) };
        }

        if let Some(engine) = s.engine.as_mut() {
            let mut pitch = if rpm > IDLE_RPM { rpm as f32 * 0.000_488_281_25 + 0.25 } else { IDLE_PITCH };
            let volume = if train {
                pitch = pitch.max(0.5) * 0.5;
                let loud = rpm as f32 * 0.007_812_5;
                if loud < 0.25 { 0.0 } else { loud.min(1.0) }
            } else {
                (rpm as f32 * 0.015625).min(0.5)
            } * 0.5;

            s.engine_volume += (sight(front) * volume - s.engine_volume) * EASE;

            engine.place(front, if train { TRAIN_RADIUS } else { ENGINE_RADIUS }, &ears);
            engine.player().set_speed(pitch);
            engine.player().set_volume(s.engine_volume * ears.volume);
        }

        if train {
            continue;
        }

        let rear = at.translation + back * 0.125 - up * 0.25;
        let skid: f32 = v.wheels.iter().map(|w| w[2]).sum();

        if s.tyres.is_none() && skid > SKIDDING {
            let first = skid.min(0.25);
            s.tyres = start(&speakers, &clips.tyres, rear, TYRE_RADIUS, &ears, first);
            s.tyre_volume = first;
        }

        if let Some(tyres) = s.tyres.as_mut() {
            s.tyre_volume += (sight(rear) * skid.min(1.0) - s.tyre_volume) * EASE;

            tyres.place(rear, TYRE_RADIUS, &ears);
            tyres.player().set_volume(s.tyre_volume * ears.volume);

            if s.tyre_volume < SKIDDING {
                s.tyres = None;
            }
        }
    }

    engines.0.retain(|s| s.engine.is_some() || s.tyres.is_some());
}
