use std::f32::consts::{FRAC_PI_2, PI};

use ferrosa_protocol::clientbound::game::ItemKind;

use crate::game::{controls::{Control, InputState, Settings, TURN, dialling, flag, inventory_input, item_kind, number_keys, scroll_actions, slot_item}, netgame::NetGame};

pub fn drive(net: &mut NetGame, me: usize, human: i32, input: &mut InputState, settings: &Settings) {
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
    p.zoom_eased += (p.zoom - p.zoom_eased) / 16.0;
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

    p.view_yaw_delta = p.view_yaw_delta.clamp(-PI, PI);
    p.view_pitch = p.view_pitch.clamp(-FRAC_PI_2, FRAC_PI_2);

    p.forward_back = if input.held(Control::Forward) {
        p.forward_back + 1.0 / 32.0
    } else if input.held(Control::Back) {
        p.forward_back - 1.0 / 32.0
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

fn steer(lr: f32, left: bool, right: bool) -> f32 {
    let ease = |lr: f32| {
        if right {
            if lr < 231.0 / 320.0 { lr + 9.0 / 320.0 } else { 0.75 }
        } else if lr > 3.0 / 80.0 {
            lr - 3.0 / 80.0
        } else {
            lr.min(0.0)
        }
    };

    let push = |lr: f32| if right { lr + 9.0 / 320.0 } else { lr };

    if left {
        if lr > -231.0 / 320.0 { ease(lr - 9.0 / 320.0) } else { push(-0.75) }
    } else if lr < -3.0 / 80.0 {
        ease(lr + 3.0 / 80.0)
    } else if lr < 0.0 {
        push(0.0)
    } else {
        ease(lr)
    }
}