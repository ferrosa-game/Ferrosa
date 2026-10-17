use std::net::SocketAddrV4;

use bevy::prelude::*;
use ferrosa_protocol::{
    CharacterCustomization, GameMode, LeaveGame, Team,
    clientbound::{
        game::{
            GameState, ItemKind, MenuType, ItemTail, VehicleKind, MenuButton, OwnHumanData, ServerGamePacket,
            decode::{ObjectDecoder, SlotChange},
            events::{Event, ServerEvent, chat::ChatType, sound::Sound},
        },
        initial_sync::InitialSync,
    },
    codec::{Reader, WireRead},
    frame_packet,
    serverbound::game::{
        ClientGamePacket,
        actions::{GameAction, Menu, MenuAction},
        voice::{VoiceData, VoiceFrame},
    },
};

use crate::{menu::join::ServerSync, net::ClientSocket};

pub const PLAYERS: usize = 0x100;
pub const HUMANS: usize = 0x100;
pub const ITEMS: usize = 0x400;
pub const VEHICLES: usize = 0x200;
pub const TRAFFIC: usize = 0x400;
const EVENTS: usize = 0x10000;
const ACTIONS: usize = 0x40;
const ACTIONS_SENT: i32 = 0xf;
const VOICE_FRAMES: usize = 0x40;
const VOICE_SENT: usize = 6;
const VOICE_RECEIVED: usize = 4;
pub const VOICE_CHANNELS: usize = 8;
pub const INVENTORY_SLOTS: usize = 7;
const MAX_ITEM_CHILDREN: usize = 8;
const BONES: usize = 0xf;
const TIMEOUT: u32 = 0x707;
const STALE: i32 = 0x270f;
const CHAT_LINES: usize = 0x40;
const CHAT_LENGTH: usize = 0x7f;
const LONG_CHAT: f32 = 10.0;
const SHORT_CHAT: f32 = 8.0;

#[derive(Clone, Debug)]
pub struct Player {
    pub active: bool,
    pub is_bot: bool,
    pub is_ready: bool,
    pub is_admin: bool,
    pub human: i32,
    pub team: Team,
    pub name: String,
    pub look: CharacterCustomization,
    pub phone_number: i32,
    pub stocks: i32,
    pub manager_tab: bool,
    pub menu: MenuType,
    pub menu_tab: i32,
    pub menu_buttons: Vec<MenuButton>,
    pub money: i32,
    pub team_money: i32,
    pub budget: i32,
    pub corporate_rating: i32,
    pub criminal_rating: i32,
    pub spawn_timer: i32,
    pub vitality: i32,
    pub display_inventory: [Vec<i32>; INVENTORY_SLOTS],
    pub last_actions: i32,
    pub actions: Vec<Option<GameAction>>,
    pub action_head: i32,
    pub gear_x: f32,
    pub left_right: f32,
    pub gear_y: f32,
    pub forward_back: f32,
    pub view_yaw_delta: f32,
    pub view_pitch: f32,
    pub free_look_yaw: f32,
    pub free_look_pitch: f32,
    pub view_yaw: f32,
    pub new_field: f32,
    pub view_pitch_delta: f32,
    pub input_flags: u32,
    pub input_type: u32,
    pub zoom_level: u32,
    pub last_input_type: u32,
    pub held: i32,
    pub zoom: f32,
    pub zoom_eased: f32,
    pub aim_pitch: [f32; 3],
}

impl Default for Player {
    fn default() -> Self {
        Self {
            active: false,
            is_bot: false,
            is_ready: false,
            is_admin: false,
            human: -1,
            team: Team::Spectator,
            name: String::new(),
            look: CharacterCustomization::default(),
            phone_number: 0,
            stocks: 0,
            manager_tab: false,
            menu: MenuType::Empty,
            menu_tab: 0,
            menu_buttons: Vec::new(),
            money: 0,
            team_money: 0,
            budget: 0,
            corporate_rating: 0,
            criminal_rating: 0,
            spawn_timer: 0,
            vitality: 0,
            display_inventory: Default::default(),
            last_actions: 0,
            actions: vec![None; ACTIONS],
            action_head: 0,
            gear_x: 0.0,
            left_right: 0.0,
            gear_y: 0.0,
            forward_back: 0.0,
            view_yaw_delta: 0.0,
            view_pitch: 0.0,
            free_look_yaw: 0.0,
            free_look_pitch: 0.0,
            view_yaw: 0.0,
            new_field: 0.0,
            view_pitch_delta: 0.0,
            input_flags: 0,
            input_type: 0,
            zoom_level: 0,
            last_input_type: 0,
            held: -1,
            zoom: 0.0,
            zoom_eased: 0.0,
            aim_pitch: [0.0; 3],
        }
    }
}

impl Player {
    pub fn queue(&mut self, a: GameAction) {
        self.actions[self.action_head as usize & (ACTIONS - 1)] = Some(a);

        self.action_head = (self.action_head + 1) & (ACTIONS as i32 - 1);
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Track {
    pub tick: i32,
    pub pos: Vec3,
    pub prev_tick: i32,
    pub prev_pos: Vec3,
}

impl Track {
    fn moved(&mut self, tick: i32, pos: Vec3) {
        *self = Self {
            tick,
            pos,
            prev_tick: self.tick,
            prev_pos: self.pos,
        };
    }

    fn reset(&mut self, tick: i32, pos: Vec3) {
        *self = Self {
            tick,
            pos,
            prev_tick: tick,
            prev_pos: pos,
        };
    }
}

#[derive(Clone, Debug)]
pub struct Human {
    pub active: bool,
    pub player: i32,
    pub look: CharacterCustomization,
    pub wanted: u32,
    pub vehicle: i32,
    pub vehicle_seat: u32,
    pub alive: bool,
    pub bleeding: bool,
    pub track: Track,
    pub rot: Quat,
    pub bones: [Quat; BONES],
    pub inventory: [Vec<i32>; INVENTORY_SLOTS],
}

impl Default for Human {
    fn default() -> Self {
        Self {
            active: false,
            player: -1,
            look: CharacterCustomization::default(),
            wanted: 0,
            vehicle: -1,
            vehicle_seat: 0,
            alive: false,
            bleeding: false,
            track: Track::default(),
            rot: Quat::IDENTITY,
            bones: [Quat::IDENTITY; BONES],
            inventory: Default::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Item {
    pub active: bool,
    pub kind: Option<ItemKind>,
    pub parent_item: i32,
    pub parent_human: i32,
    pub parent_slot: u32,
    pub track: Track,
    pub has_history: bool,
    pub rot: Quat,
    pub tail: ItemTail,
    pub phone: (i32, i32, i32),
    pub children: Vec<i32>,
}

impl Default for Item {
    fn default() -> Self {
        Self {
            active: false,
            kind: None,
            parent_item: -1,
            parent_human: -1,
            parent_slot: 0,
            track: Track::default(),
            has_history: false,
            rot: Quat::IDENTITY,
            tail: ItemTail::None,
            phone: (0, 0, 0),
            children: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Vehicle {
    pub active: bool,
    pub health: i32,
    pub kind: VehicleKind,
    pub color: u32,
    pub traffic_car: i32,
    pub track: Track,
    pub rot: Quat,
    pub steer: f32,
    pub wheels: [[f32; 3]; 4],
    pub engine_rpm: u32,
    pub windows_broken: [bool; 8],
    pub burst: u32,
}

impl Default for Vehicle {
    fn default() -> Self {
        Self {
            active: false,
            health: 0,
            kind: VehicleKind::default(),
            color: 0,
            traffic_car: -1,
            track: Track::default(),
            rot: Quat::IDENTITY,
            steer: 0.0,
            wheels: [[0.0; 3]; 4],
            engine_rpm: 0,
            windows_broken: [false; 8],
            burst: 0,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TrafficCar {
    pub kind: VehicleKind,
    pub color: u32,
    pub vehicle: i32,
    pub track: Track,
    pub yaw: f32,
}

#[derive(Clone, Debug)]
pub struct VoiceChannel {
    pub active: bool,
    pub player: i32,
    pub human: i32,
    pub item: i32,
    pub frames: Vec<VoiceFrame>,
    pub next: usize,
}

impl Default for VoiceChannel {
    fn default() -> Self {
        Self { active: false, player: -1, human: -1, item: -1, frames: vec![VoiceFrame::default(); VOICE_FRAMES], next: 0 }
    }
}

#[derive(Clone, Debug)]
pub struct ChatLine {
    pub kind: ChatType,
    pub speaker: i32,
    pub text: String,
    pub volume: i32,
    pub life: f32,
}

#[derive(Message, Clone, Debug)]
pub enum NetEvent {
    Gunshot { kind: Option<ItemKind>, item: i32, bullet: i32, pos: Vec3, vel: Vec3 },
    BulletHit { hit: i32, pos: Vec3, normal: Vec3 },
    BulletHole { pos: Vec3, vel: Vec3, cell: IVec3, face: i32 },
    Explosion { pos: Vec3 },
    WindowBroke { vehicle: i32, window: i32, pos: Vec3, vel: Vec3 },
    ShellHit { pos: Vec3, volume: f32, pitch: f32 },
    Sound { sound: Sound, pos: Vec3, volume: f32, pitch: f32 },
}

const FRESH: u32 = 0x63;

#[derive(Resource)]
pub struct NetGame {
    pub server: SocketAddrV4,
    pub round: u32,
    pub server_round: u32,
    pub sync: ServerSync,
    pub game_state: GameState,
    pub share_prices: [i32; 6],
    pub silence: u32,
    pub tick: i32,
    pub timer: u32,
    pub timer_extra: u32,
    pub sun_time: u32,
    pub team_counts: [u32; 5],
    pub corp_round: [(i32, i32); 3],
    pub player: i32,
    pub players: Vec<Player>,
    pub own_human: Option<OwnHumanData>,
    pub humans: Vec<Human>,
    pub items: Vec<Item>,
    pub vehicles: Vec<Vehicle>,
    pub traffic: Vec<TrafficCar>,
    pub traffic_count: u32,
    pub signals: Vec<[i32; 4]>,
    pub team_doors: Vec<bool>,
    pub happenings: Vec<NetEvent>,
    pub chat: Vec<ChatLine>,
    pub decoder: ObjectDecoder,
    events: Vec<Option<Event>>,
    pub events_received: u32,
    pub events_played: u32,
    pub server_events: u32,
    pub server_packet: u32,
    sent: u32,
    pub ping: u32,
    pub spectating: i32,
    pub voice_out: Vec<VoiceFrame>,
    pub voice_out_next: usize,
    pub silenced: bool,
    pub voice: Vec<VoiceChannel>,
}

impl NetGame {
    pub fn new(server: SocketAddrV4, sync: ServerSync) -> Self {
        Self {
            server,
            round: sync.round,
            server_round: sync.round,
            sync,
            game_state: GameState::default(),
            share_prices: [0; 6],
            silence: 0,
            tick: 0,
            timer: 0,
            timer_extra: 0,
            sun_time: 0,
            team_counts: [0; 5],
            corp_round: [(0, 0); 3],
            player: 0,
            players: vec![Player::default(); PLAYERS],
            own_human: None,
            humans: vec![Human::default(); HUMANS],
            items: vec![Item::default(); ITEMS],
            vehicles: vec![Vehicle::default(); VEHICLES],
            traffic: vec![
                TrafficCar {
                    vehicle: -1,
                    ..default()
                };
                TRAFFIC
            ],
            traffic_count: 0,
            signals: vec![[0; 4]; 0x400],
            team_doors: Vec::new(),
            happenings: Vec::new(),
            chat: Vec::new(),
            decoder: ObjectDecoder::default(),
            events: vec![None; EVENTS],
            events_received: 0,
            events_played: 0,
            server_events: 0,
            server_packet: 0,
            sent: 0,
            ping: 0,
            spectating: -1,
            voice_out: vec![VoiceFrame::default(); VOICE_FRAMES],
            voice_out_next: 0,
            silenced: true,
            voice: vec![VoiceChannel::default(); VOICE_CHANNELS],
        }
    }

    pub fn me(&self) -> Option<&Player> {
        usize::try_from(self.player)
            .ok()
            .and_then(|p| self.players.get(p))
    }

    pub fn me_mut(&mut self) -> Option<&mut Player> {
        usize::try_from(self.player)
            .ok()
            .and_then(|p| self.players.get_mut(p))
    }

    pub fn menu_action(&mut self, button: u32, flag: u8) {
        let Some(p) = self.me_mut() else { return };

        let menu = match p.menu {
            MenuType::Lobby => Menu::Lobby,
            other => Menu::Other(other as u8),
        };

        let c = std::iter::once(flag)
            .chain(std::iter::repeat_n(0, 15))
            .collect();

        p.queue(GameAction::Menu(MenuAction { menu, button, c }));

        p.menu_buttons.clear();
    }

    pub fn send(&mut self, socket: &mut ClientSocket, camera: Vec3, now: u32) {
        let Some(pl) = self.me() else { return };

        let (head, tail) = (pl.action_head, pl.last_actions);

        let count = if head < tail {
            head + ACTIONS as i32 - tail
        } else {
            head - tail
        }
        .min(ACTIONS_SENT);

        let actions = (0..count)
            .map(|k| {
                pl.actions[((tail + k) as usize) & (ACTIONS - 1)]
                    .clone()
                    .unwrap_or(GameAction::Unknown)
            })
            .collect();

        let frames = std::array::from_fn(|k| {
            let f = (self.voice_out_next + VOICE_FRAMES - VOICE_SENT + k) % VOICE_FRAMES;
            VoiceFrame { index: f as u8, ..self.voice_out[f].clone() }
        });

        let p = ClientGamePacket {
            round_num: self.round,
            gear_x: pl.gear_x,
            left_right: pl.left_right,
            gear_y: pl.gear_y,
            forward_back: pl.forward_back,
            view_yaw_delta: pl.view_yaw_delta,
            view_pitch: pl.view_pitch,
            free_look_yaw: pl.free_look_yaw,
            free_look_pitch: pl.free_look_pitch,
            view_yaw: pl.view_yaw,
            unknown: pl.new_field,
            view_pitch_delta: pl.view_pitch_delta,
            input_flags: pl.input_flags,
            input_type: pl.input_type as u8,
            zoom_level: pl.zoom_level as u8,
            received_events: self.events_played,
            num_sent_objects: self.decoder.object_packet,
            camera_pos: camera,
            packet_action_count: count as u8,
            total_actions: tail as u8,
            actions,
            voice_data: VoiceData {
                frames,
                is_silenced: self.silenced,
            },
            spectating_human_id: self.spectating as u8,
            pack_ack: self.decoder.packs_applied,
            link_ack: self.decoder.links_received,
            packet_count: self.sent,
            sdl_tick: now,
        };

        self.sent = self.sent.wrapping_add(1);

        socket.send(&frame_packet(p), self.server);
    }

    pub fn leave(&self, socket: &mut ClientSocket) {
        socket.send(&frame_packet(LeaveGame), self.server);
    }

    // TODO: packet 9's admin list.
    pub fn receive(&mut self, socket: &mut ClientSocket, now: u32) -> bool {
        self.silence += 1;

        if self.silence > TIMEOUT {
            return false;
        }

        while let Some(packet) = socket.recv() {
            if packet.from == std::net::SocketAddr::V4(self.server) {
                self.packet(&packet.data[4..], now);
            }
        }

        true
    }

    fn packet(&mut self, data: &[u8], now: u32) {
        let mut r = Reader::new(data);

        match r.u8() {
            Ok(6) => {
                self.silence = 0;

                if let Ok(sync) = InitialSync::read(&mut r) {
                    self.sync = ServerSync::from(sync);
                    self.server_round = self.sync.round;
                }
            }
            Ok(5) => {
                self.silence = 0;

                let (round, tick) = (self.round, self.tick);

                let accept =
                    |r: u32, t: u32| r == round && !(tick > t as i32 && tick - t as i32 <= STALE);

                if let Ok(Some(p)) = ServerGamePacket::read(&mut r, &mut self.decoder, accept) {
                    self.apply(p, now);
                }
            }
            Ok(9) => {
                if let Some(p) = self.me_mut() {
                    p.is_admin = true;
                }
            }
            _ => {}
        }
    }

    // TODO: the shop lists a store menu shows.
    fn apply(&mut self, p: ServerGamePacket, now: u32) {
        self.tick = p.ticks_since_reset as i32;
        self.game_state = p.gamestate;

        if let Some(ready) = p.ready_states {
            self.players
                .iter_mut()
                .zip(ready)
                .for_each(|(pl, r)| pl.is_ready = r);
        }

        if matches!(p.gamestate, GameState::Intermission | GameState::Restarting) {
            self.corp_round = p.corp_round;
        }

        self.timer = p.game_timer as u32;
        self.timer_extra = p.game_timer_extra as u32;
        self.sun_time = p.sun_time as u32;
        self.team_counts = p.team_counts.map(|n| n as u32);
        self.player = p.client_id as i32;
        let human = p.own_human.as_ref().map_or(-1, |h| h.human_id);
        self.own_human = p.own_human.clone();

        let my_human = &mut self.players[p.client_id as usize & (PLAYERS - 1)];

        my_human.human = human;
        my_human.manager_tab = p.manager_tab;
        my_human.menu = p.menu_type;
        my_human.menu_tab = p.menu_tab;
        my_human.money = p.money;
        my_human.team_money = p.corp_money;
        my_human.budget = p.corp_credit;
        my_human.corporate_rating = p.corp_rating;
        my_human.criminal_rating = p.crim_rating;
        my_human.spawn_timer = p.spawn_timer;
        my_human.last_actions = p.received_actions as i32;
        my_human.vitality = p.vitality;
        my_human.display_inventory = p.display_inventory.clone();
        my_human.menu_buttons = p.menu_buttons.clone();

        self.objects(&p);

        for (channel, sent) in self.voice.iter_mut().zip(&p.voice) {
            let Some(sent) = sent else {
                channel.active = false;
                channel.frames.iter_mut().for_each(|f| f.size = 0);
                continue;
            };

            (channel.active, channel.player, channel.human, channel.item) = (true, sent.player_id, sent.human_id, sent.item_id);

            for f in &sent.voice_frames {
                channel.frames[f.index as usize % VOICE_FRAMES] = f.clone();
            }

            channel.next = (sent.voice_frames[VOICE_RECEIVED - 1].index as usize + 1) % VOICE_FRAMES;
        }
        self.server_events = p.global_event_count;

        for (id, e) in p.events {
            self.events[id as usize & (EVENTS - 1)] = Some(e);
            if self.events_received == id {
                self.events_received = (self.events_received + 1) & 0xffff;
            }
        }

        self.server_packet = p.packet_sequence;
        self.ping = now.wrapping_sub(p.last_sdl_tick);

        self.rebuild_inventories();
    }

    // TODO: the line links (memo and computer lines) and the voice channels.
    fn objects(&mut self, p: &ServerGamePacket) {
        for c in std::mem::take(&mut self.decoder.changes) {
            match c {
                SlotChange::Pack { kind: 0, index, .. } => {
                    if let Some(h) = self.humans.get_mut(index as usize) {
                        h.active = true;
                    }
                }
                SlotChange::Pack {
                    kind: 1,
                    item_type,
                    index,
                } => {
                    if let Some(it) = self.items.get_mut(index as usize) {
                        *it = Item {
                            active: true,
                            kind: u8::try_from(item_type).ok().and_then(|t| ItemKind::try_from(t).ok()),
                            ..default()
                        };
                    }
                }
                SlotChange::Unpack { kind: 0, index } => {
                    if let Some(h) = self.humans.get_mut(index as usize) {
                        h.active = false;
                    }
                }
                SlotChange::Unpack { kind: 1, index } => {
                    if let Some(it) = self.items.get_mut(index as usize) {
                        it.active = false;
                    }
                }
                _ => {}
            }
        }

        let tick = self.tick;
        let quat = |q: [f32; 4]| Quat::from_xyzw(q[0], q[1], q[2], q[3]);

        for obj in &p.humans {
            let human = &mut self.humans[obj.human_id as usize & (HUMANS - 1)];

            if let Some(a) = &obj.appearance {
                human.player = a.player_id;
                human.look = a.customization;
                human.wanted = a.wanted as u32;
            }

            human.vehicle = obj.vehicle;
            human.vehicle_seat = obj.vehicle_seat as u32;
            human.alive = obj.alive;
            human.bleeding = obj.bleeding;
            human.track.moved(tick, obj.pos);
            human.rot = quat(obj.rot);
            human.bones = obj.bones.map(quat);
        }

        for object in &p.items {
            let item = &mut self.items[object.item_id as usize & (ITEMS - 1)];

            if let Some(hd) = &object.header {
                item.kind = Some(object.item_type);
                item.parent_item = hd.parent_item;
                item.parent_human = hd.parent_human;
                item.parent_slot = hd.parent_slot as u32;
            }

            if item.has_history {
                item.track.moved(tick, object.pos);
            } else {
                item.track.reset(tick, object.pos);
                item.has_history = true;
            }

            item.rot = quat(object.rot);
            item.tail = object.tail;
        }

        for obj in &p.vehicles {
            let id = obj.vehicle_id as usize & (VEHICLES - 1);
            let car = usize::try_from(obj.traffic_car).ok().filter(|&c| c < TRAFFIC);
            let vehicle = &mut self.vehicles[id];

            vehicle.traffic_car = obj.traffic_car;
            vehicle.track.moved(tick, obj.pos);
            vehicle.rot = quat(obj.rot);

            if let Some(car) = car.filter(|&c| vehicle.active && self.traffic[c].vehicle != id as i32) {
                let t = &mut self.traffic[car];
                t.vehicle = id as i32;
                vehicle.kind = t.kind;
                vehicle.color = t.color;
                vehicle.track.reset(tick, obj.pos);
            }

            vehicle.steer = (obj.steer as f32 as f64 * std::f64::consts::PI / 255.0) as f32;
            vehicle.wheels = obj.wheels.map(|w| w.map(|v| v as f32 / 255.0));
            vehicle.engine_rpm = obj.engine_rpm as u32;
        }

        self.traffic_count = p.traffic_count as u32;

        for obj in &p.traffic {
            let traffic_car = &mut self.traffic[obj.index as usize & (TRAFFIC - 1)];
            traffic_car.track.moved(tick, obj.pos);

            if !obj.vehicle {
                traffic_car.vehicle = -1;
            }

            if let Some((kind, color)) = obj.kind_color {
                traffic_car.kind = kind;
                traffic_car.color = color as u32;
            }

            traffic_car.yaw = obj.yaw;

            let (vehicle, kind, color) = (traffic_car.vehicle, traffic_car.kind, traffic_car.color);
            if let Some(ve) = usize::try_from(vehicle)
                .ok()
                .and_then(|v| self.vehicles.get_mut(v))
            {
                ve.kind = kind;
                ve.color = color;
            }
        }

        if let Some(s) = self.signals.get_mut(p.signal.0 as usize) {
            *s = p.signal.1;
        }
    }

    // TODO: the sounds and effects events make, cells, missions, stocks and the mute list.
    pub fn process_events(&mut self) {
        while self.events_played != self.events_received {
            if let Some(e) = self.events[self.events_played as usize].take() {
                self.event(e);
            }

            self.events_played = (self.events_played + 1) & 0xffff;
        }
    }

    fn happening(&self, kind: &ServerEvent) -> Option<NetEvent> {
        Some(match kind {
            ServerEvent::Bullet(e) => NetEvent::Gunshot { kind: self.items.get(e.item_id as usize).and_then(|i| i.kind), item: e.item_id, bullet: e.bullet_type, pos: e.pos, vel: e.vel },
            ServerEvent::BulletHit(e) => NetEvent::BulletHit { hit: e.hit_type, pos: e.pos, normal: e.normal },
            ServerEvent::BulletHole(e) => NetEvent::BulletHole { pos: e.pos, vel: e.vel, cell: IVec3::new(e.block_x, e.block_y, e.block_z), face: e.face },
            ServerEvent::Explosion(e) => NetEvent::Explosion { pos: e.pos },
            ServerEvent::UpdateVehicle(e) if e.kind == 0 => NetEvent::WindowBroke { vehicle: e.vehicle_id, window: e.part, pos: e.pos, vel: e.velocity },
            ServerEvent::Sound(e) => NetEvent::Sound { sound: e.sound_type, pos: e.pos, volume: e.volume, pitch: e.pitch },
            ServerEvent::PhoneSound(e) => {
                let item = self.items.get(e.item_id as usize).filter(|i| i.active)?;
                NetEvent::Sound { sound: e.sound, pos: item.track.pos, volume: e.volume, pitch: e.pitch }
            }
            _ => return None,
        })
    }

    fn event(&mut self, ev: Event) {
        let fresh = self.tick.wrapping_sub(ev.tick_created as i32) as u32 <= FRESH;

        if let Some(happening) = self.happening(&ev.kind).filter(|_| fresh) {
            self.happenings.push(happening);
        }

        match ev.kind {
            ServerEvent::Chat(event) => {
                if fresh || matches!(event.chat_type, ChatType::EliminatorAnnouncement | ChatType::Notice) {
                    info!("chat {:?}: {}", event.chat_type, event.message);

                    let life = if matches!(event.chat_type, ChatType::Announce | ChatType::AdminChat) { LONG_CHAT } else { SHORT_CHAT };

                    self.chat.push(ChatLine {
                        kind: event.chat_type,
                        speaker: event.speaker_id,
                        text: event.message.chars().take(CHAT_LENGTH).collect(),
                        volume: event.volume,
                        life,
                    });

                    if self.chat.len() > CHAT_LINES {
                        self.chat.remove(0);
                    }
                }
            }
            ServerEvent::UpdateVehicleTypeColor(event) => {
                if let Some(vehicle) = self.vehicles.get_mut(event.vehicle_id as usize) {
                    vehicle.active = true;
                    vehicle.health = 100;
                    vehicle.kind = event.vehicle_type;

                    vehicle.color = if event.vehicle_color == 0xf {
                        u32::MAX
                    } else {
                        event.vehicle_color as u32
                    };

                    vehicle.track.tick = -1;
                    vehicle.windows_broken = [false; 8];
                    vehicle.burst = 0;
                }
            }
            ServerEvent::UpdateVehicle(event) => {
                let Some(vehicle) = self.vehicles.get_mut(event.vehicle_id as usize) else {
                    return;
                };

                match event.kind {
                    0 | 6 => {
                        if let Some(w) = usize::try_from(event.part)
                            .ok()
                            .and_then(|w| vehicle.windows_broken.get_mut(w))
                        {
                            *w = event.kind == 0;
                        }
                    }
                    1 => vehicle.burst |= u32::try_from(event.part).map_or(0, |w| 1u32.wrapping_shl(w)),
                    2 => vehicle.health = event.part,
                    3 => {
                        if let Some(t) = usize::try_from(vehicle.traffic_car)
                            .ok()
                            .and_then(|c| self.traffic.get_mut(c))
                        {
                            t.vehicle = -1;
                        }

                        vehicle.active = false;
                    }
                    _ => {}
                }
            }
            ServerEvent::UpdateItem(event) => {
                let id = event.item_id as usize & (ITEMS - 1);
                let item = &mut self.items[id];

                if event.active && !item.active {
                    item.track = Track::default();
                }

                item.active = event.active;
                item.kind = ItemKind::try_from(event.item_type).ok();
                item.parent_human = event.parent_human;
                item.parent_item = event.parent_item;
                item.parent_slot = event.parent_slot as u32;

                self.decoder.set_item_kind(id, event.item_type as u32);
            }
            ServerEvent::UpdatePhone(event) => {
                self.items[event.item_id as usize & (ITEMS - 1)].phone =
                    (event.phone_status, event.display_phone_number, event.phone_texture);
            }
            ServerEvent::UpdatePlayer(event) => {
                let player = &mut self.players[event.client_id as usize & (PLAYERS - 1)];

                player.active = event.active;

                if player.active {
                    player.is_bot = event.is_bot;
                    player.team = event.team;
                    player.human = event.human_id;
                    player.look = event.customization;
                    player.name = event.name;
                }
            }
            ServerEvent::UpdateStock(event) => {
                let [a, b, c] = event.prices;
                self.share_prices[..5].copy_from_slice(&[a & 0xffff, a >> 16, b & 0xffff, b >> 16, c & 0xffff]);
            }
            ServerEvent::UpdatePlayerRound(event) => {
                let player = &mut self.players[event.client_id as usize & (PLAYERS - 1)];

                player.money = event.money;
                player.stocks = event.stocks;
                player.phone_number = event.phone_number as i32;
            }
            ServerEvent::TeamDoor(event) => {
                if let Ok(team) = usize::try_from(event.team_id) {
                    if self.team_doors.len() <= team {
                        self.team_doors.resize(team + 1, false);
                    }

                    self.team_doors[team] = event.door_open;
                }
            }
            ServerEvent::UpdateHuman(event) => {
                let Some(human) = self.humans.get_mut(event.human_id as usize) else {
                    return;
                };

                human.active = event.active;

                if let Some(p) = usize::try_from(event.player_id)
                    .ok()
                    .and_then(|p| self.players.get_mut(p))
                {
                    p.human = if event.active { event.human_id } else { -1 };
                }

                if event.active {
                    human.player = event.player_id;
                }
            }
            _ => {}
        }
    }

    pub fn reset_round(&mut self) {
        self.round = self.server_round;
        self.tick = 0;
        self.events_received = 0;
        self.events_played = 0;
        self.events.iter_mut().for_each(|e| *e = None);
        self.decoder = ObjectDecoder::default();
        self.spectating = -1;
        self.team_doors.clear();

        let menu = match self.sync.game_type {
            GameMode::Racing | GameMode::Round | GameMode::Eliminator | GameMode::CoOp | GameMode::Versus => MenuType::Lobby,
            _ => MenuType::Empty,
        };

        for p in &mut self.players {
            if p.is_bot {
                p.active = false;
            }

            *p = Player {
                active: p.active,
                is_bot: p.is_bot,
                is_admin: p.is_admin,
                team: p.team,
                name: std::mem::take(&mut p.name),
                look: p.look,
                phone_number: p.phone_number,
                menu,
                ..default()
            };
        }

        self.humans.iter_mut().for_each(|h| h.active = false);
        self.vehicles.iter_mut().for_each(|v| v.active = false);
        self.items.iter_mut().for_each(|i| i.active = false);
        self.traffic.iter_mut().for_each(|t| t.vehicle = -1);
        self.traffic_count = 0;
        self.chat.retain(|c| c.kind != ChatType::Announce);
    }

    fn rebuild_inventories(&mut self) {
        self.humans
            .iter_mut()
            .for_each(|h| h.inventory.iter_mut().for_each(Vec::clear));

        self.items.iter_mut().for_each(|i| i.children.clear());

        for i in 0..ITEMS {
            let it = &self.items[i];

            if !it.active {
                continue;
            }

            let (parent, human, slot) = (it.parent_item, it.parent_human, it.parent_slot as usize);

            if let Some(p) = usize::try_from(parent)
                .ok()
                .map(|p| &mut self.items[p & (ITEMS - 1)])
                .filter(|p| p.children.len() < MAX_ITEM_CHILDREN)
            {
                p.children.push(i as i32);
            }

            if let Some(inv) = usize::try_from(human)
                .ok()
                .filter(|_| slot < INVENTORY_SLOTS)
                .map(|h| &mut self.humans[h & (HUMANS - 1)].inventory[slot])
                && inv.len() <= 1
            {
                inv.push(i as i32);
            }
        }
    }
}
