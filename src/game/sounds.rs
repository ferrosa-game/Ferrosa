use std::f64::consts::PI;

use bevy::{
    audio::{AudioSource, SpatialScale, Volume},
    platform::collections::HashMap,
    prelude::*,
};
use ferrosa_protocol::clientbound::game::{ItemKind, events::sound::Sound};

use super::{camera::GameCamera, netgame::NetEvent};
use crate::{config::Config, menu::Screen};

const EAR_GAP: f32 = 0.2;
const RATE: u32 = 48000;
const GUNS: [(ItemKind, &str); 5] = [(ItemKind::Ak47, "ak47"), (ItemKind::M16, "m16"), (ItemKind::Mp5, "mp5"), (ItemKind::Uzi, "uzi"), (ItemKind::Pistol, "pistol")];
const SHOTS: usize = 6;
const RICOCHETS: usize = 8;
const NEAR: f32 = 1.0;
const SHORT: f32 = 10.0;
const FAR: f32 = 32.0;
const KEYS: f32 = 0.25;
const RICOCHET: i32 = 0;
const BODY: i32 = 1;
const METAL: i32 = 2;
const GLASS_PITCH: (f32, f32) = (0.75, 0.75 + 255.0 / 512.0);
const KEY_LENGTH: usize = 0x4000;
const BUSY_LENGTH: usize = 0x5622;
const FADE: usize = 0x20;

#[derive(Clone)]
struct Clip {
    source: Handle<AudioSource>,
    radius: f32,
}

#[derive(Resource)]
struct GameSounds {
    guns: HashMap<ItemKind, Vec<Clip>>,
    ricochets: Vec<Clip>,
    body: Vec<Clip>,
    metal: Vec<Clip>,
    glass: Clip,
    explosion: Clip,
    server: HashMap<u8, Clip>,
}

pub struct SoundsPlugin;

impl Plugin for SoundsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_sounds)
            .add_systems(Update, (listen, play_sounds).run_if(in_state(Screen::InGame)));
    }
}

fn load_sounds(mut commands: Commands, assets: Res<AssetServer>, mut sources: ResMut<Assets<AudioSource>>) {
    let file = |name: &str, radius: f32| Clip { source: assets.load(format!("sound/{name}.wav")), radius };
    let numbered = |stem: &str, count: usize, radius: f32| (1..=count).map(|k| file(&format!("{stem}{k:02}"), radius)).collect::<Vec<_>>();
    let mut tone = |key: usize| Clip { source: sources.add(AudioSource { bytes: wav(&dial_tone(key)).into() }), radius: KEYS };

    let mut server: HashMap<u8, Clip> = [
        (Sound::CarCrash, file("carcrash02", FAR)),
        (Sound::BodyHit, file("bullethitbody02", SHORT)),
        (Sound::PhoneRing, file("phonering01", NEAR)),
        (Sound::Reload, file("reload01", NEAR)),
        (Sound::GearShift, file("gearshift01", NEAR)),
        (Sound::Modem, file("modem", NEAR)),
        (Sound::Floppy, file("floppy", NEAR)),
    ]
    .into_iter()
    .map(|(sound, clip)| (sound as u8, clip))
    .collect();

    for (key, &sound) in Sound::PHONE_KEYS.iter().enumerate() {
        server.insert(sound as u8, tone(key));
    }

    server.insert(Sound::PhoneBusy as u8, tone(10));

    commands.insert_resource(GameSounds {
        guns: GUNS.iter().map(|&(kind, name)| (kind, numbered(&format!("{name}_fire"), SHOTS, SHORT))).collect(),
        ricochets: numbered("ricochet", RICOCHETS, SHORT),
        body: numbered("bullethitbody", 2, SHORT),
        metal: numbered("bullethitmetal", 2, SHORT),
        glass: file("glassbreak01", SHORT),
        explosion: file("explosion01", FAR),
        server,
    });
}

fn dial_tone(key: usize) -> Vec<i16> {
    let (high, low, length) = match key {
        1 => (1209.0, 697.0, KEY_LENGTH),
        2 => (1336.0, 697.0, KEY_LENGTH),
        3 => (1477.0, 697.0, KEY_LENGTH),
        4 => (1209.0, 770.0, KEY_LENGTH),
        5 => (1336.0, 770.0, KEY_LENGTH),
        6 => (1477.0, 770.0, KEY_LENGTH),
        7 => (1209.0, 852.0, KEY_LENGTH),
        8 => (1336.0, 852.0, KEY_LENGTH),
        9 => (1477.0, 852.0, KEY_LENGTH),
        0 => (1336.0, 941.0, KEY_LENGTH),
        _ => (480.0, 620.0, BUSY_LENGTH),
    };
    let step = |f: f64| f * 2.0 * PI / RATE as f64;

    (0..length)
        .map(|i| {
            let fade = 0.9f64.powi(i.saturating_sub(length - FADE) as i32);
            let t = i as f64;
            (((step(high) * t).sin() + (step(low) * t).sin()) * fade * 8192.0) as i16
        })
        .collect()
}

fn wav(samples: &[i16]) -> Vec<u8> {
    let data = (samples.len() * 2) as u32;
    let header = [
        &b"RIFF"[..],
        &(36 + data).to_le_bytes(),
        b"WAVEfmt ",
        &16u32.to_le_bytes(),
        &1u16.to_le_bytes(),
        &1u16.to_le_bytes(),
        &RATE.to_le_bytes(),
        &(RATE * 2).to_le_bytes(),
        &2u16.to_le_bytes(),
        &16u16.to_le_bytes(),
        b"data",
        &data.to_le_bytes(),
    ]
    .concat();

    header.into_iter().chain(samples.iter().flat_map(|s| s.to_le_bytes())).collect()
}

fn listen(mut commands: Commands, cameras: Query<Entity, (With<GameCamera>, Without<SpatialListener>)>) {
    for camera in &cameras {
        commands.entity(camera).insert(SpatialListener::new(EAR_GAP));
    }
}

fn pick(clips: &[Clip]) -> Option<&Clip> {
    clips.get(rand::random_range(0..clips.len().max(1)))
}

fn play_sounds(mut commands: Commands, mut happenings: MessageReader<NetEvent>, sounds: Option<Res<GameSounds>>, config: Res<Config>) {
    let Some(sounds) = sounds else { return };
    let master = if config.sound != 0 { config.sound_volume as f32 / 100.0 } else { 0.0 };

    for happening in happenings.read() {
        let (clip, pos, volume, pitch) = match *happening {
            NetEvent::Gunshot { kind, pos, .. } => (kind.and_then(|k| sounds.guns.get(&k)).and_then(|c| pick(c)), pos, 1.0, 1.0),
            NetEvent::BulletHit { hit, pos, .. } => {
                let clips = match hit {
                    RICOCHET => &sounds.ricochets[..],
                    BODY => &sounds.body,
                    METAL => &sounds.metal,
                    _ => &[],
                };
                (pick(clips), pos, 1.0, 1.0)
            }
            NetEvent::BulletHole { pos, glass: true } | NetEvent::WindowBroke { pos } => (Some(&sounds.glass), pos, 1.0, rand::random_range(GLASS_PITCH.0..GLASS_PITCH.1)),
            NetEvent::Explosion { pos } => (Some(&sounds.explosion), pos, 1.0, 1.0),
            NetEvent::Sound { sound, pos, volume, pitch } => (sounds.server.get(&(sound as u8)), pos, volume, pitch),
            NetEvent::BulletHole { glass: false, .. } => (None, Vec3::ZERO, 0.0, 1.0),
        };

        let Some(clip) = clip else { continue };
        let settings = PlaybackSettings::DESPAWN
            .with_spatial(true)
            .with_spatial_scale(SpatialScale::new(clip.radius.sqrt().recip()))
            .with_volume(Volume::Linear(volume * master))
            .with_speed(pitch);

        commands.spawn((AudioPlayer(clip.source.clone()), settings, Transform::from_translation(pos), DespawnOnExit(Screen::InGame)));
    }
}
