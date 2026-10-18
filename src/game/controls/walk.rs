use std::{f64::consts::{PI, TAU}, f64::consts::FRAC_PI_2};

use ferrosa_protocol::clientbound::game::ItemKind;

use crate::game::{controls::{Control, InputState, MAX_YAW, Settings, TURN, angle_difference, dialling, first_row, flag, inventory_input, item_kind, number_keys, scroll_actions, slot_item}, netgame::{NetGame, entities::Player}};

pub fn walk(net: &mut NetGame, me: usize, human: i32, input: &mut InputState, settings: &Settings) {
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

    p.spin_yaw = (p.spin_yaw + turn * 0.5) * 0.9375;
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

    p.view_yaw_delta = p.view_yaw_delta.clamp(-MAX_YAW as f32, MAX_YAW as f32);
    p.view_pitch = p.view_pitch.clamp(-FRAC_PI_2 as f32, FRAC_PI_2 as f32);
    p.free_look_yaw = p.free_look_yaw.clamp(-MAX_YAW as f32, MAX_YAW as f32);
    p.free_look_pitch = p.free_look_pitch.clamp(-FRAC_PI_2 as f32, FRAC_PI_2 as f32);

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

fn view_pitch(p: &mut Player, input: &InputState, settings: &Settings) {
    if !input.left {
        p.aim_pitch[0] += input.dy * settings.pitch_scale;
    }

    p.aim_pitch[0] = p.aim_pitch[0].clamp(-FRAC_PI_2 as f32, FRAC_PI_2 as f32);

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
    p.gear_x = (angle_difference(p.gear_x, door) as f64).clamp(-FRAC_PI_2, FRAC_PI_2) as f32 + door;
}