use bevy::prelude::*;
use ferrosa_protocol::{CharacterCustomization, Team, clientbound::game::{ItemKind, ItemTail, MenuButton, MenuType, VehicleKind}, serverbound::game::actions::GameAction};

use crate::game::netgame::{ACTIONS, BONES, INVENTORY_SLOTS, MEMO_SIZE};

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
    pub muted: bool,
    pub is_speaking: bool,
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
    pub spin_yaw: f32,
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
            muted: false,
            is_speaking: false,
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
            spin_yaw: 0.0,
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
    pub fn moved(&mut self, tick: i32, pos: Vec3) {
        *self = Self {
            tick,
            pos,
            prev_tick: self.tick,
            prev_pos: self.pos,
        };
    }

    pub fn reset(&mut self, tick: i32, pos: Vec3) {
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
    pub memo: Vec<u8>,
    pub screen: Vec<(Vec<u8>, Vec<u8>)>,
    pub current_line: i32,
    pub top_line: i32,
    pub cursor: i32,
    pub text_version: u32,
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
            memo: vec![0; MEMO_SIZE],
            screen: Vec::new(),
            current_line: 0,
            top_line: 0,
            cursor: -1,
            text_version: 0,
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