use std::f64::consts::{FRAC_PI_2 as HALF_PI, PI, TAU};

use bevy::{platform::collections::HashSet, prelude::*};
use bevy_enhanced_input::prelude::*;
use ferrosa_protocol::{
    clientbound::game::{ItemKind, ItemTail, VehicleKind},
    serverbound::game::actions::{GameAction, InventoryAction, ItemAction},
};

use super::{
    escape_menu::GameMenu,
    first_person::FirstPerson,
    netgame::{NetGame, Player},
    network::NetTick,
    vehicles::VehicleTypes,
};
use crate::{config::Config, menu::Screen};

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
const MAX_YAW: f64 = 2.7488935718912502;
const TURN: f32 = 0.001953125;
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

struct Settings {
    invert: bool,
    manual_hands: bool,
    pitch_scale: f32,
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
    player: Query<(Entity, &ContextActivity<PlayerControls>)>,
    spectator: Query<(Entity, &ContextActivity<SpectatorControls>)>,
) {
    let playing = menu.is_some_and(|m| *m.get() == GameMenu::Closed);

    for (e, activity) in &player {
        if **activity != (playing && first_person.active) {
            commands.entity(e).insert(ContextActivity::<PlayerControls>::new(playing && first_person.active));
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

fn angle_difference(mut a: f32, mut b: f32) -> f32 {
    if !(((b - a) as f64) < PI) {
        a = (a as f64 + TAU) as f32;
    }
    if !(((a - b) as f64) < PI) {
        b = (b as f64 + TAU) as f32;
    }
    a - b
}

fn clamp_to(v: &mut f32, limit: f64) {
    let d = *v as f64;
    if !(-limit <= d) {
        *v = -limit as f32;
    } else if !(d <= limit) {
        *v = limit as f32;
    }
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

fn inventory_input(net: &mut NetGame, me: usize, human: i32, input: &mut InputState, settings: &Settings) {
    let shift = input.held(Control::Shift);
    let hand = if settings.manual_hands { shift as u32 } else if shift { 2 } else { 3 };
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

fn view_pitch(p: &mut Player, input: &InputState, settings: &Settings) {
    if !input.left {
        p.aim_pitch[0] += input.dy * settings.pitch_scale;
    }

    clamp_to(&mut p.aim_pitch[0], HALF_PI);

    let (target, eased, speed) = (p.aim_pitch[0], p.aim_pitch[1], p.aim_pitch[2]);
    let d = ((target - eased) * 0.125).clamp(-0.03125, 0.03125);

    p.aim_pitch[2] = d - speed * 0.25 + speed;
    p.aim_pitch[1] = p.aim_pitch[2] + eased;
    p.free_look_pitch = target * 0.125;
}

fn swing_door(net: &mut NetGame, me: usize, item: i32, input: &InputState) {
    let view_yaw = net.own_human.as_ref().map_or(0.0, |o| o.view_yaw);
    let (x, y) = (1.25 * input.dx * 0.0009765625, 1.25 * input.dy * 0.0009765625);
    let p = &mut net.players[me];
    let (gs, gc) = p.gear_x.sin_cos();
    let (vs, vc) = view_yaw.sin_cos();
    p.gear_x = (x * vs + gs + y * vc).atan2(x * vc + gc - y * vs);
    let (m0, m2) = first_row(net.items[item as usize].rot);
    let door = (m2 as f64).atan2(m0 as f64) as f32;
    let p = &mut net.players[me];
    p.gear_x = (angle_difference(p.gear_x, door) as f64).clamp(-HALF_PI, HALF_PI) as f32 + door;
}

fn walk(net: &mut NetGame, me: usize, human: i32, input: &mut InputState, settings: &Settings) {
    let (body_pitch, body_yaw) = net.own_human.as_ref().map_or((0.0, 0.0), |o| (o.view_pitch, o.body_yaw));
    let p = &mut net.players[me];

    p.left_right = 0.0;
    p.gear_y = 0.0;
    p.forward_back = 0.0;
    p.input_flags = 0;

    if p.last_input_type != 1 {
        p.gear_x = 0.0;
        p.view_pitch = body_pitch;
        p.view_yaw = body_yaw;
    }

    if settings.paused {
        return;
    }

    let in_hand = slot_item(net, human, input.held(Control::Shift) as usize);

    if in_hand >= 0 && net.players[me].held != in_hand && item_kind(net, in_hand) == Some(ItemKind::Door) {
        let (m0, m2) = first_row(net.items[in_hand as usize].rot);
        net.players[me].gear_x = (m2 as f64).atan2(m0 as f64) as f32;
    }

    net.players[me].held = in_hand;

    if item_kind(net, in_hand) == Some(ItemKind::CashWorld) {
        scroll_actions(net, me, in_hand, input);
    }

    let door = item_kind(net, in_hand) == Some(ItemKind::Door);
    let p = &mut net.players[me];

    p.zoom_level = (p.zoom_level as i32 + input.wheel).clamp(0, 2) as u32;

    input.wheel = 0;

    p.zoom = [0.0, 0.0625, 0.5][p.zoom_level as usize];
    p.zoom_eased += (p.zoom - p.zoom_eased) * 0.0625;

    let slow = 1.0 - p.zoom_eased;
    let dx = slow * input.dx;
    let turn = dx * TURN;
    let tilt = if settings.invert { -1.0 } else { 1.0 } * slow * input.dy * TURN;

    p.new_field = (p.new_field + turn * 0.5) * 0.9375;
    p.view_pitch_delta = (p.view_pitch_delta + tilt * 0.5) * 0.9375;

    let follow = |p: &mut Player| {
        let behind = angle_difference(body_yaw, p.view_yaw);
        p.view_yaw = body_yaw;
        behind
    };

    if input.free_look {
        let behind = follow(p);

        p.view_yaw_delta = angle_difference(p.view_yaw_delta, behind);
        p.free_look_yaw += turn;
        p.free_look_pitch += tilt;
        p.input_flags |= flag::MANUAL;
    } else if in_hand >= 0 && input.aim {
        let behind = follow(p);
        p.view_yaw_delta = ((p.view_yaw_delta - behind) as f64 + PI).rem_euclid(TAU) as f32 - PI as f32 + dx * 0.00048828125;

        view_pitch(p, input, settings);

        if input.left {
            p.view_pitch += tilt * 0.5;
        }
    } else if door {
        swing_door(net, me, in_hand, input);

        let p = &mut net.players[me];

        p.view_yaw_delta += 0.125 * p.free_look_yaw;
        p.view_pitch += 0.125 * p.free_look_pitch;
        p.free_look_yaw *= 0.875;
        p.free_look_pitch *= 0.875;
    } else {
        let behind = follow(p);

        p.view_yaw_delta = p.view_yaw_delta - behind + turn + 0.125 * p.free_look_yaw;
        p.view_pitch += tilt + 0.125 * p.free_look_pitch;
        p.free_look_yaw *= 0.875;
        p.free_look_pitch *= 0.875;
        p.aim_pitch = [0.0; 3];
    }

    let p = &mut net.players[me];

    clamp_to(&mut p.view_yaw_delta, MAX_YAW);
    clamp_to(&mut p.view_pitch, HALF_PI);
    clamp_to(&mut p.free_look_yaw, MAX_YAW);
    clamp_to(&mut p.free_look_pitch, HALF_PI);

    let axis = |plus: Control, minus: Control| input.held(plus) as i32 as f32 - input.held(minus) as i32 as f32;

    p.forward_back = axis(Control::Forward, Control::Back);
    p.left_right = axis(Control::Right, Control::Left);

    let keys = [
        (Control::Jump, flag::JUMP),
        (Control::Reload, flag::RELOAD),
        (Control::Delete, flag::DELETE),
        (Control::Shift, flag::SHIFT),
        (Control::Crouch, flag::CROUCH),
        (Control::Ragdoll, flag::RAGDOLL),
        (Control::Use, flag::USE),
        (Control::Special, flag::F),
    ];

    p.input_flags |= keys.iter().filter(|(k, _)| input.held(*k)).fold(0, |f, (_, b)| f | b) | if input.aim { flag::AIM } else { 0 };

    if dialling(net, in_hand) {
        number_keys(&mut net.players[me], in_hand, input);
    }

    inventory_input(net, me, human, input, settings);
}

fn steer(lr: f32, left: bool, right: bool) -> f32 {
    let ease = |lr: f32| {
        if right {
            if lr < 0.721875 { lr + 0.028125 } else { 0.75 }
        } else if lr > 0.0375 {
            lr - 0.0375
        } else {
            lr.min(0.0)
        }
    };

    let push = |lr: f32| if right { lr + 0.028125 } else { lr };

    if left {
        if lr > -0.721875 { ease(lr - 0.028125) } else { push(-0.75) }
    } else if lr < -0.0375 {
        ease(lr + 0.0375)
    } else if lr < 0.0 {
        push(0.0)
    } else {
        ease(lr)
    }
}

fn drive(net: &mut NetGame, me: usize, human: i32, input: &mut InputState, settings: &Settings) {
    let held = slot_item(net, human, 0);
    let p = &mut net.players[me];

    p.input_flags = 0;

    if settings.paused {
        return;
    }
    p.zoom_level = 1;

    if p.last_input_type != 2 {
        p.view_yaw_delta = 0.0;
        p.view_pitch = 0.0;
        p.gear_x = 2.0;
        p.gear_y = -1.0;
    }

    if item_kind(net, held) == Some(ItemKind::CashWorld) {
        scroll_actions(net, me, held, input);
    }

    let p = &mut net.players[me];
    p.zoom = (p.zoom + input.wheel as f32 * 0.125).clamp(0.0, 0.625);
    input.wheel = 0;
    p.zoom_eased += (p.zoom - p.zoom_eased) * 0.0625;
    if input.right {
        p.gear_x = (p.gear_x + input.dx / 150.0).clamp(-0.125, 2.125);
        p.gear_y = (p.gear_y + input.dy / 150.0).clamp(-1.0, 1.0);
        p.input_flags |= flag::MANUAL;
    } else {
        let slow = 1.0 - p.zoom_eased;
        p.view_yaw_delta += slow * input.dx * TURN;
        p.view_pitch += if settings.invert { -1.0 } else { 1.0 } * slow * input.dy * TURN;
        p.gear_x = if p.gear_x < 0.5 { 0.0 } else if p.gear_x >= 1.5 { 2.0 } else { 1.0 };
        p.gear_y = if p.gear_y < -0.375 { -1.0 } else if p.gear_y <= 0.375 { 0.0 } else { 1.0 };
    }

    clamp_to(&mut p.view_yaw_delta, PI);
    clamp_to(&mut p.view_pitch, HALF_PI);

    p.forward_back = if input.held(Control::Forward) {
        p.forward_back + 0.03125
    } else if input.held(Control::Back) {
        p.forward_back - 0.03125
    } else if p.forward_back >= 0.125 {
        p.forward_back - 0.125
    } else if p.forward_back <= -0.125 {
        p.forward_back + 0.125
    } else {
        0.0
    };

    p.left_right = steer(p.left_right, input.held(Control::Left), input.held(Control::Right));

    let keys = [
        (Control::Jump, flag::JUMP),
        (Control::Crouch, flag::CROUCH),
        (Control::Shift, flag::SHIFT),
        (Control::Use, flag::USE),
        (Control::Reload, flag::RELOAD),
        (Control::Special, flag::F),
        (Control::Delete, flag::DELETE),
    ];

    p.input_flags |= keys.iter().filter(|(k, _)| input.held(*k)).fold(0, |f, (_, b)| f | b);

    if item_kind(net, held) == Some(ItemKind::Phone) && dialling(net, held) {
        number_keys(&mut net.players[me], held, input);
    }

    inventory_input(net, me, human, input, settings);

    let p = &mut net.players[me];
    p.forward_back = p.forward_back.clamp(-1.0, 1.0);
}

fn fly(net: &mut NetGame, me: usize, human: i32, input: &mut InputState, settings: &Settings) {
    let p = &mut net.players[me];

    if p.last_input_type != 3 {
        p.forward_back = 0.0;
        p.view_yaw_delta = 0.0;
        p.view_pitch = 0.0;
    }

    p.zoom_level = 1;
    p.gear_x = 0.0;
    p.left_right = 0.0;
    p.gear_y = 0.0;
    p.input_flags = 0;

    if settings.paused {
        return;
    }

    if input.right && !input.left {
        let slow = 1.0 - p.zoom_eased;
        p.view_yaw_delta += slow * input.dx / 650.0;
        p.view_pitch += slow * input.dy / 650.0;

        clamp_to(&mut p.view_yaw_delta, MAX_YAW);
        clamp_to(&mut p.view_pitch, HALF_PI);
    } else {
        for s in &mut input.stick {
            *s = if *s < -1.0 { *s + 1.0 } else if *s <= 1.0 { 0.0 } else { *s - 1.0 };
        }

        input.stick[0] += 0.0625 * input.dx;
        input.stick[1] += 0.0625 * input.dy;

        p.gear_y = input.stick[0];
        p.gear_x -= input.stick[1];

        if !input.left {
            p.view_yaw_delta *= 0.875;
            p.view_pitch *= 0.875;
        }
    }

    p.zoom = (p.zoom + input.wheel as f32 * 0.125).clamp(0.0, 0.875);
    input.wheel = 0;
    p.zoom_eased += (p.zoom - p.zoom_eased) * 0.0625;

    let axis = |plus: Control, minus: Control| input.held(plus) as i32 as f32 - input.held(minus) as i32 as f32;
    p.forward_back += 0.03125 * axis(Control::Forward, Control::Back);
    p.left_right += axis(Control::Right, Control::Left);

    let keys = [(Control::Jump, flag::JUMP), (Control::Crouch, flag::CROUCH), (Control::Shift, flag::SHIFT), (Control::Use, flag::USE)];
    p.input_flags |= keys.iter().filter(|(k, _)| input.held(*k)).fold(0, |f, (_, b)| f | b);

    inventory_input(net, me, human, input, settings);

    let p = &mut net.players[me];

    for v in [&mut p.gear_x, &mut p.left_right, &mut p.gear_y, &mut p.forward_back] {
        *v = v.clamp(-1.0, 1.0);
    }
}

// TODO: C and M's map views.
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

    let settings = Settings {
        invert: config.invert_mouse != 0,
        manual_hands: config.manual_hands != 0,
        pitch_scale: config.mouse_scale.1,
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
                walk(net, me, human, input, &settings);
                1
            }
            Some(Some((1, _) | (_, 1..))) => {
                drive(net, me, human, input, &settings);
                2
            }
            Some(Some(_)) => {
                fly(net, me, human, input, &settings);
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
