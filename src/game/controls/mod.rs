use std::f64::consts::PI;

use bevy::{platform::collections::HashSet, prelude::*};
use bevy_enhanced_input::prelude::*;
use ferrosa_protocol::{
    clientbound::game::{ItemKind, ItemTail, VehicleKind},
    serverbound::game::actions::{GameAction, InventoryAction, ItemAction},
};

use super::{
    computer::AtComputer,
    escape_menu::GameMenu,
    first_person::FirstPerson,
    netgame::{NetGame, entities::Player},
    network::NetTick,
    vehicles::VehicleTypes,
};
use crate::{config::Config, menu::Screen};

pub mod walk;
pub mod drive;
pub mod fly;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Control {
    Forward,
    Back,
    Left,
    Right,
    Jump,
    Crouch,
    Shift,
    Use,
    Reload,
    Special,
    Delete,
    Ragdoll,
    Aim,
    Primary,
    Secondary,
    Row(u8),
    Pad(u8),
}

pub trait IsControl: InputAction {
    const CONTROL: Control;
}

macro_rules! controls {
    ($($action:ident => $control:expr),* $(,)?) => {
        $(
            #[derive(InputAction)]
            #[action_output(bool)]
            pub struct $action;

            impl IsControl for $action {
                const CONTROL: Control = $control;
            }
        )*

        fn gather_buttons(app: &mut App) {
            $(app.add_systems(PreUpdate, gather::<$action>.after(EnhancedInputSystems::Update));)*
        }
    };
}

controls! {
    Forward => Control::Forward,
    Back => Control::Back,
    StepLeft => Control::Left,
    StepRight => Control::Right,
    Jump => Control::Jump,
    Crouch => Control::Crouch,
    Shift => Control::Shift,
    Use => Control::Use,
    Reload => Control::Reload,
    Special => Control::Special,
    Delete => Control::Delete,
    Ragdoll => Control::Ragdoll,
    Aim => Control::Aim,
    Primary => Control::Primary,
    Secondary => Control::Secondary,
    Row0 => Control::Row(0),
    Row1 => Control::Row(1),
    Row2 => Control::Row(2),
    Row3 => Control::Row(3),
    Row4 => Control::Row(4),
    Row5 => Control::Row(5),
    Row6 => Control::Row(6),
    Row7 => Control::Row(7),
    Row8 => Control::Row(8),
    Row9 => Control::Row(9),
    Pad0 => Control::Pad(0),
    Pad1 => Control::Pad(1),
    Pad2 => Control::Pad(2),
    Pad3 => Control::Pad(3),
    Pad4 => Control::Pad(4),
    Pad5 => Control::Pad(5),
    Pad6 => Control::Pad(6),
    Pad7 => Control::Pad(7),
    Pad8 => Control::Pad(8),
    Pad9 => Control::Pad(9),
}

#[derive(InputAction)]
#[action_output(Vec2)]
pub struct Look;

#[derive(InputAction)]
#[action_output(Vec2)]
pub struct Zoom;

#[derive(InputAction)]
#[action_output(Vec3)]
pub struct Fly;

#[derive(InputAction)]
#[action_output(Vec2)]
pub struct FlyLook;

#[derive(InputAction)]
#[action_output(Vec2)]
pub struct FlyZoom;

#[derive(InputAction)]
#[action_output(bool)]
pub struct FlyFast;

#[derive(InputAction)]
#[action_output(bool)]
pub struct FlySlow;

#[derive(Component, Default, Clone)]
pub struct PlayerControls;

#[derive(Component, Default, Clone)]
pub struct SpectatorControls;

pub(super) mod flag {
    pub const FIRE: u32 = 1;
    pub const MANUAL: u32 = 2;
    pub const JUMP: u32 = 4;
    pub const CROUCH: u32 = 8;
    pub const SHIFT: u32 = 0x10;
    pub const AIM: u32 = 0x20;
    pub const USE: u32 = 0x800;
    pub const RELOAD: u32 = 0x1000;
    pub const F: u32 = 0x2000;
    pub const DELETE: u32 = 0x40000;
    pub const RAGDOLL: u32 = 0x80000;
}

const PHONE_DIALLING: i32 = 1;
const MAX_YAW: f64 = 7.0 * PI / 8.0;
const TURN: f32 = 1.0 / 512.0;
const SLOT_KEYS: [Control; 5] = [Control::Row(1), Control::Row(2), Control::Row(3), Control::Row(4), Control::Row(5)];
const DIAL_KEYS: [Control; 20] = [
    Control::Pad(0),
    Control::Pad(1),
    Control::Pad(2),
    Control::Pad(3),
    Control::Pad(4),
    Control::Pad(5),
    Control::Pad(6),
    Control::Pad(7),
    Control::Pad(8),
    Control::Pad(9),
    Control::Row(0),
    Control::Row(1),
    Control::Row(2),
    Control::Row(3),
    Control::Row(4),
    Control::Row(5),
    Control::Row(6),
    Control::Row(7),
    Control::Row(8),
    Control::Row(9),
];

#[derive(Resource, Default)]
pub(super) struct InputState {
    gathered_held: HashSet<Control>,
    gathered_pressed: HashSet<Control>,
    gathered_look: Vec2,
    gathered_wheel: f32,
    dx: f32,
    dy: f32,
    wheel: i32,
    left: bool,
    right: bool,
    prev_left: bool,
    prev_right: bool,
    keys: HashSet<Control>,
    prev_keys: HashSet<Control>,
    aim: bool,
    prev_aim: bool,
    free_look: bool,
    firing: bool,
    stick: [f32; 2],
}

impl InputState {
    pub(super) fn holding(&self, k: Control) -> bool {
        self.gathered_held.contains(&k)
    }

    pub(super) fn aiming(&self) -> bool {
        self.aim
    }

    fn held(&self, k: Control) -> bool {
        self.keys.contains(&k)
    }

    fn just_pressed(&self, k: Control) -> bool {
        self.keys.contains(&k) && !self.prev_keys.contains(&k)
    }

    fn released(&self, k: Control) -> bool {
        !self.keys.contains(&k) && self.prev_keys.contains(&k)
    }

    fn next_tick(&mut self) {
        let held = self.gathered_held.iter().copied().chain(self.gathered_pressed.drain()).collect();
        let wheel = self.gathered_wheel.trunc();

        self.prev_keys = std::mem::replace(&mut self.keys, held);
        (self.prev_left, self.prev_right) = (self.left, self.right);
        self.left = self.held(Control::Primary);
        self.right = self.held(Control::Secondary);
        (self.dx, self.dy) = std::mem::take(&mut self.gathered_look).into();
        self.wheel = wheel as i32;
        self.gathered_wheel -= wheel;
    }
}

pub struct Settings {
    paused: bool,
}

pub struct ControlsPlugin;

impl Plugin for ControlsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EnhancedInputPlugin)
            .add_input_context::<PlayerControls>()
            .add_input_context::<SpectatorControls>()
            .init_resource::<InputState>()
            .add_systems(OnEnter(Screen::InGame), spawn_controls)
            .add_systems(OnExit(Screen::InGame), |mut input: ResMut<InputState>| *input = default())
            .add_systems(Update, switch_controls.run_if(in_state(Screen::InGame)))
            .add_systems(PreUpdate, gather_movement.after(EnhancedInputSystems::Update))
            .add_systems(FixedUpdate, read_input.in_set(NetTick::Input));

        gather_buttons(app);
    }
}

fn spawn_controls(mut commands: Commands) {
    commands
        .spawn_scene(bsn! {
            PlayerControls
            Actions<PlayerControls> [
                Action::<Forward>::new() Bindings [ Binding(KeyCode::KeyW) ]
                --
                Action::<Back>::new() Bindings [ Binding(KeyCode::KeyS) ]
                --
                Action::<StepLeft>::new() Bindings [ Binding(KeyCode::KeyA) ]
                --
                Action::<StepRight>::new() Bindings [ Binding(KeyCode::KeyD) ]
                --
                Action::<Jump>::new() Bindings [ Binding(KeyCode::Space) ]
                --
                Action::<Crouch>::new() Bindings [ Binding(KeyCode::ControlLeft) ]
                --
                Action::<Shift>::new() Bindings [ Binding(KeyCode::ShiftLeft) ]
                --
                Action::<Use>::new() Bindings [ Binding(KeyCode::KeyE) ]
                --
                Action::<Reload>::new() Bindings [ Binding(KeyCode::KeyR) ]
                --
                Action::<Special>::new() Bindings [ Binding(KeyCode::KeyF) ]
                --
                Action::<Delete>::new() Bindings [ Binding(KeyCode::Delete) ]
                --
                Action::<Ragdoll>::new() Bindings [ Binding(KeyCode::KeyZ) ]
                --
                Action::<Aim>::new() Bindings [ Binding(KeyCode::KeyQ) ]
                --
                Action::<Primary>::new() Bindings [ Binding(MouseButton::Left) ]
                --
                Action::<Secondary>::new() Bindings [ Binding(MouseButton::Right) ]
                --
                Action::<Row0>::new() Bindings [ Binding(KeyCode::Digit0) ]
                --
                Action::<Row1>::new() Bindings [ Binding(KeyCode::Digit1) ]
                --
                Action::<Row2>::new() Bindings [ Binding(KeyCode::Digit2) ]
                --
                Action::<Row3>::new() Bindings [ Binding(KeyCode::Digit3) ]
                --
                Action::<Row4>::new() Bindings [ Binding(KeyCode::Digit4) ]
                --
                Action::<Row5>::new() Bindings [ Binding(KeyCode::Digit5) ]
                --
                Action::<Row6>::new() Bindings [ Binding(KeyCode::Digit6) ]
                --
                Action::<Row7>::new() Bindings [ Binding(KeyCode::Digit7) ]
                --
                Action::<Row8>::new() Bindings [ Binding(KeyCode::Digit8) ]
                --
                Action::<Row9>::new() Bindings [ Binding(KeyCode::Digit9) ]
                --
                Action::<Pad0>::new() Bindings [ Binding(KeyCode::Numpad0) ]
                --
                Action::<Pad1>::new() Bindings [ Binding(KeyCode::Numpad1) ]
                --
                Action::<Pad2>::new() Bindings [ Binding(KeyCode::Numpad2) ]
                --
                Action::<Pad3>::new() Bindings [ Binding(KeyCode::Numpad3) ]
                --
                Action::<Pad4>::new() Bindings [ Binding(KeyCode::Numpad4) ]
                --
                Action::<Pad5>::new() Bindings [ Binding(KeyCode::Numpad5) ]
                --
                Action::<Pad6>::new() Bindings [ Binding(KeyCode::Numpad6) ]
                --
                Action::<Pad7>::new() Bindings [ Binding(KeyCode::Numpad7) ]
                --
                Action::<Pad8>::new() Bindings [ Binding(KeyCode::Numpad8) ]
                --
                Action::<Pad9>::new() Bindings [ Binding(KeyCode::Numpad9) ]
                --
                Action::<Look>::new() Bindings [ Binding::mouse_motion() ]
                --
                Action::<Zoom>::new() Bindings [ Binding::mouse_wheel() ]
            ]
        })
        .insert(DespawnOnExit(Screen::InGame));

    commands
        .spawn_scene(bsn! {
            SpectatorControls
            Actions<SpectatorControls> [
                Action::<Fly>::new()
                Bindings [ {preset::spatial::new(KeyCode::KeyW, KeyCode::KeyS, KeyCode::KeyA, KeyCode::KeyD, KeyCode::KeyQ, KeyCode::KeyZ)} ]
                --
                Action::<FlyLook>::new() Bindings [ Binding::mouse_motion() ]
                --
                Action::<FlyZoom>::new() Bindings [ Binding::mouse_wheel() ]
                --
                Action::<FlyFast>::new() Bindings [ Binding(KeyCode::ShiftLeft) ]
                --
                Action::<FlySlow>::new() Bindings [ Binding(KeyCode::ControlLeft) ]
            ]
        })
        .insert(DespawnOnExit(Screen::InGame));
}

fn switch_controls(
    mut commands: Commands,
    menu: Option<Res<State<GameMenu>>>,
    first_person: Res<FirstPerson>,
    at_computer: Res<AtComputer>,
    player: Query<(Entity, &ContextActivity<PlayerControls>)>,
    spectator: Query<(Entity, &ContextActivity<SpectatorControls>)>,
) {
    let playing = menu.is_some_and(|m| *m.get() == GameMenu::Closed);
    let walking = playing && first_person.active && !at_computer.typing();

    for (e, activity) in &player {
        if **activity != walking {
            commands.entity(e).insert(ContextActivity::<PlayerControls>::new(walking));
        }
    }

    for (e, activity) in &spectator {
        if **activity != (playing && !first_person.active) {
            commands.entity(e).insert(ContextActivity::<SpectatorControls>::new(playing && !first_person.active));
        }
    }
}

fn gather<A: IsControl>(actions: Query<&TriggerState, With<Action<A>>>, mut input: ResMut<InputState>) {
    if actions.iter().any(|s| *s == TriggerState::Fired) {
        input.gathered_held.insert(A::CONTROL);
        input.gathered_pressed.insert(A::CONTROL);
    } else {
        input.gathered_held.remove(&A::CONTROL);
    }
}

fn gather_movement(look: Query<&Action<Look>>, zoom: Query<&Action<Zoom>>, mut input: ResMut<InputState>) {
    input.gathered_look += look.iter().map(|a| **a).sum::<Vec2>();
    input.gathered_wheel += zoom.iter().map(|a| a.y).sum::<f32>();
}

fn first_row(q: Quat) -> (f32, f32) {
    let (yy, zz) = (2.0 * q.y * q.y, 2.0 * q.z * q.z);
    (1.0 - (yy + zz), 2.0 * q.x * q.z + 2.0 * q.w * q.y)
}

fn key_action(p: &mut Player, item: i32, key: u32) {
    p.queue(GameAction::Item(ItemAction { a: item as u16, b: key as u16 }));
}

fn inventory_action(p: &mut Player, a: u32, b: u32, c: u32) {
    p.queue(GameAction::Inventory(InventoryAction { a: a as u16, b: b as u16, c: c as u16 }));
}

fn slot_item(net: &NetGame, human: i32, slot: usize) -> i32 {
    usize::try_from(human).ok().and_then(|h| net.humans[h].inventory[slot].first().copied()).unwrap_or(-1)
}

fn item_kind(net: &NetGame, item: i32) -> Option<ItemKind> {
    usize::try_from(item).ok().and_then(|i| net.items[i].kind)
}

fn dialling(net: &NetGame, item: i32) -> bool {
    matches!(item_kind(net, item), Some(ItemKind::Phone | ItemKind::PhonePay)) && net.items[item as usize].phone.0 == PHONE_DIALLING
}

fn number_keys(p: &mut Player, item: i32, input: &InputState) {
    for key in DIAL_KEYS {
        if let (true, Control::Pad(digit) | Control::Row(digit)) = (input.released(key), key) {
            key_action(p, item, digit as u32);
        }
    }
}

fn scroll_actions(net: &mut NetGame, me: usize, item: i32, input: &mut InputState) {
    let spread = match net.items[item as usize].tail {
        ItemTail::Cash { spread, .. } => spread as u32,
        _ => 0,
    };
    let p = &mut net.players[me];
    for _ in 0..input.wheel.max(0) {
        key_action(p, item, 0);
    }
    for _ in 0..(-input.wheel).max(0) {
        key_action(p, item, 1);
    }
    input.wheel = 0;
    if input.just_pressed(Control::Reload) {
        key_action(p, item, spread + 2);
    }
}

fn inventory_input(net: &mut NetGame, me: usize, human: i32, input: &mut InputState) {
    let shift = input.held(Control::Shift);
    let hand = if shift { 2 } else { 3 };
    let dialling = dialling(net, slot_item(net, human, 0));
    let p = &mut net.players[me];

    if input.released(Control::Reload) {
        inventory_action(p, 1, hand, hand ^ 1);
    }

    if input.just_pressed(Control::Use) {
        inventory_action(p, 8, hand, hand ^ 1);
    }

    if !dialling {
        for (k, slot) in SLOT_KEYS.into_iter().zip(3..) {
            if input.released(k) {
                inventory_action(p, slot, hand, 0);
            }
        }
    }

    if !input.aim && input.prev_aim {
        inventory_action(p, 2, hand, u32::MAX);
    }

    if input.aim && input.right && !input.prev_right {
        input.aim = false;
    }

    if !input.left {
        input.firing = false;
    } else if !input.prev_left && !input.aim {
        input.firing = true;
    }

    if input.left && input.firing {
        p.input_flags |= flag::FIRE;
    }
}

fn read_input(
    mut net: ResMut<NetGame>,
    mut state: ResMut<InputState>,
    config: Res<Config>,
    types: Option<Res<VehicleTypes>>,
    menu: Option<Res<State<GameMenu>>>,
) {
    let (net, input) = (&mut *net, &mut *state);
    let types = types.as_deref();

    input.next_tick();

    let sensitivity = config.sensitivity();
    (input.dx, input.dy) = (input.dx * sensitivity, input.dy * sensitivity);

    let settings = Settings {
        paused: menu.is_some_and(|m| *m.get() != GameMenu::Closed),
    };

    if let Ok(me) = usize::try_from(net.player) {
        input.prev_aim = input.aim;
        input.aim = input.just_pressed(Control::Aim) || (input.aim && input.held(Control::Aim));
        input.free_look = !input.aim && input.right;

        let human = net.players[me].human;
        let alive = usize::try_from(human).ok().is_some_and(|h| net.humans[h].alive);
        let controllable = |kind: VehicleKind| types.and_then(|t| t.get(kind)).map_or(1, |t| t.controllable_state);
        let getting_about = alive.then(|| usize::try_from(net.humans[human as usize].vehicle).ok().map(|v| (controllable(net.vehicles[v].kind), net.humans[human as usize].vehicle_seat)));

        let input_type = match getting_about {
            None => {
                stand_still(&mut net.players[me]);
                0
            }
            Some(None) => {
                walk::walk(net, me, human, input, &settings);
                1
            }
            Some(Some((1, _) | (_, 1..))) => {
                drive::drive(net, me, human, input, &settings);
                2
            }
            Some(Some(_)) => {
                fly::fly(net, me, human, input, &settings);
                3
            }
        };

        let p = &mut net.players[me];
        p.input_type = input_type;
        p.last_input_type = input_type;
    }
}

fn stand_still(p: &mut Player) {
    p.gear_x = 0.0;
    p.left_right = 0.0;
    p.gear_y = 0.0;
    p.forward_back = 0.0;
    p.input_flags = 0;
    p.zoom_eased = 0.0;
    p.zoom = 0.0;
}
