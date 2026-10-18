use std::{
    collections::VecDeque,
    num::NonZero,
    sync::{Arc, Mutex},
    time::Duration,
};

use bevy::prelude::*;
use ferrosa_protocol::clientbound::game::GameState;
use ferrosa_world::{skeleton::BoneId, trace::line_intersect_level};
use rodio::{ChannelCount, SampleRate, Source};
use rusty_opus::{Application, OpusDecoder, OpusEncoder};

use super::{
    Level,
    chat::ChatState,
    hud::{Stick, align, rect, spawn_screen},
    human_sync::{NetHuman, bone_at},
    netgame::{NetGame, VOICE_CHANNELS},
    network::NetTick,
    smoothing::Smoothing,
    sounds::{Ears, Emitter},
};
use crate::{
    audio::{MIC_RATE, MicSamples, Speakers},
    config::Config,
    character_model::HumanPose,
    menu::{
        Screen,
        options::{mouse_button_bit, scancode},
    },
};

const FRAME: usize = 960;
const MAX_DECODE: usize = 0x800;
const BITRATE: i32 = 16000;
const MAX_PACKET: usize = 0x800;
const CAPTURES: usize = 16;
const MAX_QUIET: i32 = 0x40;
const ACTIVATION_TAIL: i32 = 0xf;
const HOLD_TAIL: i32 = 0x17;
const DECODES: usize = 8;
const STREAM_FULL: usize = 0x167f;
const MAX_LAG: usize = 0x10;
const CATCH_UP: usize = 8;
const RING: usize = 0x40;
const SIGHT_RANGE: f32 = 128.0;
const BLOCKED: f32 = 0.5;
const SIGHT_EASE: f32 = 0.125;
const THROUGH_ITEM: f32 = 1.0;
const REACH: [f32; 3] = [2.0, 6.0, 16.0];
const QUIETEST_DB: f32 = -48.0;
const METER_FALL: f32 = 0.85;
const METER_AT: Vec2 = Vec2::new(948.0, 36.0);
const METER: Vec2 = Vec2::new(56.0, 8.0);
const METER_GAP: f32 = 16.0;
const LIT: Color = Color::srgb(0.3, 0.9, 0.3);
const UNLIT: Color = Color::srgba(1.0, 1.0, 1.0, 0.2);
const THRESHOLD: Color = Color::srgba(1.0, 1.0, 1.0, 0.8);
const MIC: Color = Color::WHITE;

#[derive(Resource, Default)]
struct MicLevel(f32);

#[derive(Component)]
struct MeterScreen;

#[derive(Component)]
struct MeterFill;

#[derive(Component)]
struct MeterThreshold;

struct Encoder {
    opus: Option<OpusEncoder>,
    quiet: i32,
    pcm: Vec<f32>,
    packet: Vec<u8>,
}

impl Encoder {
    fn encode(&mut self) -> Vec<u8> {
        let Some(opus) = self.opus.as_mut() else { return Vec::new() };

        opus.encode(&self.pcm, FRAME, &mut self.packet).map(|n| self.packet[..n].to_vec()).unwrap_or_default()
    }
}

#[derive(Resource)]
struct Microphone(Mutex<Encoder>);

impl Default for Microphone {
    fn default() -> Self {
        let opus = OpusEncoder::new(MIC_RATE as i32, 1, Application::Voip).ok().map(|mut e| {
            e.bitrate_bps = BITRATE;
            e
        });

        Self(Mutex::new(Encoder { opus, quiet: 0, pcm: vec![0.0; FRAME], packet: vec![0; MAX_PACKET] }))
    }
}

#[derive(Clone)]
struct Stream(Arc<Mutex<VecDeque<f32>>>);

impl Iterator for Stream {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        Some(self.0.lock().ok().and_then(|mut s| s.pop_front()).unwrap_or(0.0))
    }
}

impl Source for Stream {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZero::<u16>::MIN
    }

    fn sample_rate(&self) -> SampleRate {
        NonZero::new(MIC_RATE).expect("a rate")
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

struct Channel {
    decoder: Option<OpusDecoder>,
    stream: Stream,
    emitter: Option<Emitter>,
    speaker: Option<i32>,
    read: usize,
    sight: f32,
    out: Vec<f32>,
}

impl Default for Channel {
    fn default() -> Self {
        Self { decoder: None, stream: Stream(Arc::default()), emitter: None, speaker: None, read: 0, sight: 1.0, out: vec![0.0; MAX_DECODE] }
    }
}

#[derive(Resource)]
struct Voices(Mutex<Vec<Channel>>);

impl Default for Voices {
    fn default() -> Self {
        Self(Mutex::new((0..VOICE_CHANNELS).map(|_| Channel::default()).collect()))
    }
}

pub struct VoicePlugin;

impl Plugin for VoicePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Microphone>()
            .init_resource::<Voices>()
            .init_resource::<MicLevel>()
            .add_systems(OnEnter(Screen::InGame), spawn_meter)
            .add_systems(Update, show_meter.run_if(in_state(Screen::InGame)).run_if(resource_exists::<NetGame>))
            .add_systems(FixedUpdate, record.in_set(NetTick::Input))
            .add_systems(Update, play_voices.after(Smoothing::View).run_if(in_state(Screen::InGame)).run_if(resource_exists::<NetGame>))
            .add_systems(OnExit(Screen::InGame), |voices: Res<Voices>| voices.0.lock().map(|mut v| v.iter_mut().for_each(|c| *c = Channel::default())).unwrap_or_default());
    }
}

fn talking(config: &Config, keys: &ButtonInput<KeyCode>, mouse: &ButtonInput<MouseButton>) -> bool {
    let key = config.voice_key > 0 && keys.get_pressed().any(|&k| scancode(k) == Some(config.voice_key));
    let button = config.voice_mb > 0 && mouse.get_pressed().filter_map(|&b| mouse_button_bit(b)).any(|bit| config.voice_mb & (1 << bit) != 0);

    key || button
}

fn record(
    mut net: ResMut<NetGame>,
    mic: Res<Microphone>,
    samples: Res<MicSamples>,
    config: Res<Config>,
    chat: Res<ChatState>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut level: ResMut<MicLevel>,
) {
    let (Ok(mut encoder), Ok(mut captured)) = (mic.0.lock(), samples.0.lock()) else { return };
    let holding = config.voice_hold_to_talk != 0;
    let pressed = talking(&config, &keys, &mouse);

    level.0 *= METER_FALL;

    for _ in 0..CAPTURES {
        if captured.len() < FRAME {
            break;
        }

        let frame: Vec<i16> = captured.drain(..FRAME).collect();
        let loud = frame.iter().any(|&s| (s as i32).abs() > config.voice_min);
        let peak = frame.iter().map(|&s| (s as i32).abs()).max().unwrap_or(0);

        level.0 = level.0.max(meter(peak));

        encoder.pcm.iter_mut().zip(&frame).for_each(|(p, &s)| *p = s as f32 / 32768.0);

        let speaking = if holding { pressed } else { loud };
        let tail = if holding { HOLD_TAIL } else { ACTIVATION_TAIL };

        encoder.quiet = if speaking { 0 } else { (encoder.quiet + 1).min(MAX_QUIET) };

        let silent = encoder.quiet > tail;
        let data = if silent { Vec::new() } else { encoder.encode() };
        let slot = net.voice_out_next;

        net.voice_out[slot].size = data.len() as u16;
        net.voice_out[slot].data = data;
        net.voice_out[slot].volume = chat.level as u8;
        net.silenced = silent;
        net.voice_out_next = (slot + 1) % RING;
    }
}

fn meter(amplitude: i32) -> f32 {
    let ratio = amplitude.max(1) as f32 / 32768.0;

    ((20.0 * ratio.log10() - QUIETEST_DB) / -QUIETEST_DB).clamp(0.0, 1.0)
}

fn spawn_meter(mut commands: Commands) {
    let screen = spawn_screen(&mut commands, MeterScreen, Stick::TopRight);
    let mic = |commands: &mut Commands, at: Vec2, size: Vec2, round: f32| {
        let node = Node { border_radius: BorderRadius::all(px(round)), ..rect(align::TOP_LEFT, METER_AT + at, size) };
        commands.spawn((node, BackgroundColor(MIC), Pickable::IGNORE, ChildOf(screen)));
    };

    mic(&mut commands, Vec2::new(3.0, -4.0), Vec2::new(6.0, 10.0), 3.0);
    mic(&mut commands, Vec2::new(5.0, 7.0), Vec2::new(2.0, 3.0), 0.0);
    mic(&mut commands, Vec2::new(2.0, 10.0), Vec2::new(8.0, 2.0), 1.0);

    let bar = METER_AT + Vec2::X * METER_GAP;
    commands.spawn((rect(align::TOP_LEFT, bar, METER), BackgroundColor(UNLIT), Pickable::IGNORE, ChildOf(screen)));
    commands.spawn((rect(align::TOP_LEFT, bar, Vec2::new(0.0, METER.y)), MeterFill, BackgroundColor(LIT), Pickable::IGNORE, ChildOf(screen)));
    commands.spawn((rect(align::TOP_LEFT, bar - Vec2::Y * 2.0, Vec2::new(1.0, METER.y + 4.0)), MeterThreshold, BackgroundColor(THRESHOLD), Pickable::IGNORE, ChildOf(screen)));
}

fn show_meter(
    net: Res<NetGame>,
    config: Res<Config>,
    level: Res<MicLevel>,
    mut screen: Single<&mut Visibility, (With<MeterScreen>, Without<MeterThreshold>)>,
    mut fill: Single<&mut Node, (With<MeterFill>, Without<MeterThreshold>)>,
    threshold: Single<(&mut Node, &mut Visibility), With<MeterThreshold>>,
) {
    screen.set_if_neq(if net.silenced { Visibility::Hidden } else { Visibility::Inherited });

    let width = px(METER.x * level.0);

    if fill.width != width {
        fill.width = width;
    }

    let (mut mark, mut shown) = threshold.into_inner();
    let activation = config.voice_hold_to_talk == 0;
    let left = px(METER_AT.x + METER_GAP + METER.x * meter(config.voice_min));

    shown.set_if_neq(if activation { Visibility::Inherited } else { Visibility::Hidden });

    if mark.left != left {
        mark.left = left;
    }
}

pub fn line_of_sight(level: &Level, from: Vec3, to: Vec3) -> f32 {
    let map = &level.map;
    let clear = from.distance(to) <= SIGHT_RANGE && line_intersect_level(&map.ground, &map.level.area, &map.level.meshes, from, to).is_none();

    if clear { 1.0 } else { BLOCKED }
}

fn play_voices(
    mut net: ResMut<NetGame>,
    voices: Res<Voices>,
    speakers: Res<Speakers>,
    ears: Res<Ears>,
    config: Res<Config>,
    level: Option<Res<Level>>,
    humans: Query<(&NetHuman, &Transform, &HumanPose)>,
) {
    let muted = |net: &NetGame, player: i32| usize::try_from(player).ok().and_then(|p| net.players.get(p)).is_some_and(|p| p.muted);
    let speaking: Vec<i32> = net.voice.iter().filter(|c| c.active && !muted(&net, c.player)).map(|c| c.player).collect();
    let (me, silenced) = (net.player, net.silenced);

    for (k, p) in net.players.iter_mut().enumerate() {
        p.is_speaking = speaking.contains(&(k as i32)) || (k as i32 == me && !silenced);
    }

    let net = &*net;
    let (Ok(mut channels), Some(mixer)) = (voices.0.lock(), speakers.0.as_ref()) else { return };
    let restarting = net.game_state == GameState::Restarting;
    let listener = (ears.left + ears.right) * 0.5;

    for (channel, sent) in channels.iter_mut().zip(&net.voice) {
        if !sent.active || muted(net, sent.player) {
            *channel = Channel::default();
            continue;
        }

        if channel.speaker != Some(sent.player) {
            channel.decoder = OpusDecoder::new(MIC_RATE as i32, 1).ok();
            channel.read = (sent.next + RING - CATCH_UP) % RING;
            channel.speaker = Some(sent.player);
            channel.stream.0.lock().map(|mut s| s.clear()).ok();
        }

        let head = humans.iter().find(|(nh, ..)| nh.0 as i32 == sent.human).map(|(_, at, pose)| bone_at(at.translation, pose, BoneId::Head.index()).0);
        let item = usize::try_from(sent.item).ok().and_then(|i| net.items.get(i)).filter(|i| i.active).map(|i| i.track.pos);
        let at = head.filter(|_| !restarting).or(item).unwrap_or(listener);

        for _ in 0..DECODES {
            let full = channel.stream.0.lock().map_or(true, |s| s.len() > STREAM_FULL);

            if full || channel.read == sent.next {
                break;
            }

            let frame = &sent.frames[channel.read];
            let data = &frame.data[..(frame.size as usize).min(frame.data.len())];

            if let Some(decoder) = channel.decoder.as_mut() {
                let decoded = decoder.decode(data, if data.is_empty() { FRAME } else { MAX_DECODE }, &mut channel.out).unwrap_or(0);
                let silence = data.is_empty();

                if let Ok(mut stream) = channel.stream.0.lock() {
                    stream.extend(channel.out[..decoded.min(MAX_DECODE)].iter().map(|&s| if silence { 0.0 } else { s }));
                }
            }

            channel.read = (channel.read + 1) % RING;
        }

        if (sent.next + RING - channel.read) % RING > MAX_LAG {
            channel.read = (sent.next + RING - CATCH_UP) % RING;
        }

        let newest = &sent.frames[(sent.next + RING - 1) % RING];
        let radius = if item.is_some() && head.is_none() { THROUGH_ITEM } else { REACH[(newest.volume as usize).min(REACH.len() - 1)] };
        let sight = level.as_ref().map_or(1.0, |l| line_of_sight(l, at, listener));

        channel.sight += (sight - channel.sight) * SIGHT_EASE;

        let emitter = channel.emitter.get_or_insert_with(|| {
            let emitter = Emitter::new(mixer, at, radius, &ears);
            emitter.player().append(channel.stream.clone());
            emitter
        });

        emitter.place(at, radius, &ears);
        emitter.player().set_volume(channel.sight * ears.volume * config.voice_gain);
    }
}
