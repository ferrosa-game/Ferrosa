use std::{collections::HashMap, net::SocketAddrV4};

use bevy::prelude::*;
use ferrosa_protocol::{
    GameMode, Team,
    clientbound::
        game::{
            GameState, ItemKind, MenuType, OwnHumanData, decode::ObjectDecoder,
            events::{Event, chat::ChatType, sound::Sound},
        }
    ,
    serverbound::game::{
        actions::{GameAction, Menu, MenuAction},
        voice::VoiceFrame,
    },
};

pub mod entities;
pub mod send;
pub mod receive;
pub mod events;
pub mod sync;

use crate::game::netgame::{entities::{Human, Item, Player, TrafficCar, Vehicle}, sync::ServerSync};

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
const GAP: u32 = 250;
const CHAT_LINES: usize = 0x40;
const CHAT_LENGTH: usize = 0x7f;
const LONG_CHAT: f32 = 10.0;
const MUTE_LIST: usize = 0x400;
const SHORT_CHAT: f32 = 8.0;

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

#[derive(Clone, Copy, Debug)]
pub enum CellChange {
    Taken { cell: IVec3, mask: u32 },
    ItemSet { cell: IVec3, value: u32 },
}

#[derive(Clone, Debug)]
pub struct ChatLine {
    pub kind: ChatType,
    pub speaker: i32,
    pub text: String,
    pub volume: i32,
    pub life: f32,
}

#[derive(Message, Clone, Debug, PartialEq)]
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
const LINKS: usize = 0x4001;
const MEMO_SIZE: usize = 0x400;
const MEMO_LINE: usize = 0x40;
const SCREEN_LINES: usize = 0x20;
const SCREEN_COLUMNS: usize = 0x3f;
const ADMIN_ROWS: usize = 0x100;
const ADMIN_NAME: usize = 0x20;
pub const MISSION_SLOTS: usize = 0x10;
const MISSION_CORPS: usize = 7;
const RESULTS: usize = 8;
const ROLE: i32 = 0;
const TARGET: i32 = 1;
const RESULT: i32 = 2;
pub const PROTECTOR: i32 = 2;

#[derive(Clone, Debug, Default)]
pub struct Mission {
    pub active: bool,
    pub kind: i32,
    pub disk_type: i32,
    pub team1: i32,
    pub team2: i32,
    pub value: i32,
    pub deadline: i32,
    pub location: i32,
    pub cash: i32,
}

#[derive(Clone, Debug, Default)]
pub struct AdminRow {
    pub filled: bool,
    pub account: i32,
    pub name: String,
    pub phone_number: i32,
}

#[derive(Resource)]
pub struct NetGame {
    pub server: SocketAddrV4,
    pub round: u32,
    pub server_round: u32,
    pub sync: ServerSync,
    pub game_state: GameState,
    pub share_prices: [i32; 6],
    pub silence: u32,
    pub dropped: u32,
    pub last_applied: u32,
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
    pub cell_changes: Vec<CellChange>,
    pub happenings: Vec<NetEvent>,
    pub chat: Vec<ChatLine>,
    pub mute_list: Vec<i32>,
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
    link_ticks: Vec<Option<i32>>,
    pub shops: HashMap<i32, Vec<(i32, i32, i32)>>,
    pub admin_list: Vec<AdminRow>,
    pub missions: Vec<Vec<Mission>>,
    pub results: [[i32; 3]; RESULTS],
    pub result_count: usize,
    pub role: i32,
    pub saviors: (i32, i32),
    pub target: Option<Vec3>,
    pub seen_state: Option<GameState>,
    pub seen_team: Option<Team>,
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
            dropped: 0,
            last_applied: 0,
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
            cell_changes: Vec::new(),
            happenings: Vec::new(),
            chat: Vec::new(),
            mute_list: Vec::new(),
            link_ticks: vec![None; LINKS],
            shops: HashMap::new(),
            admin_list: vec![AdminRow::default(); ADMIN_ROWS],
            missions: vec![vec![Mission::default(); MISSION_SLOTS]; MISSION_CORPS],
            results: [[0; 3]; RESULTS],
            result_count: 0,
            role: -1,
            saviors: (0, 0),
            target: None,
            seen_state: None,
            seen_team: None,
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

    pub fn alive(&self) -> bool {
        self.game_state != GameState::Restarting && self.me().and_then(|p| usize::try_from(p.human).ok()).and_then(|h| self.humans.get(h)).is_some_and(|h| h.active && h.alive)
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

    pub fn reset_round(&mut self) {
        self.round = self.server_round;
        self.tick = 0;
        self.events_received = 0;
        self.events_played = 0;
        self.events.iter_mut().for_each(|e| *e = None);
        self.decoder = ObjectDecoder::default();
        self.spectating = -1;
        self.team_doors.clear();
        self.cell_changes.clear();
        self.link_ticks.iter_mut().for_each(|t| *t = None);
        self.missions.iter_mut().flatten().for_each(|m| m.active = false);
        self.result_count = 0;
        self.role = -1;
        self.target = None;

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
}
