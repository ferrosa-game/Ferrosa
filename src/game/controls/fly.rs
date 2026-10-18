use std::f32::consts::FRAC_PI_2;

use crate::game::{controls::{Control, InputState, MAX_YAW, Settings, flag, inventory_input}, netgame::NetGame};

pub fn fly(net: &mut NetGame, me: usize, human: i32, input: &mut InputState, settings: &Settings) {
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
        p.view_pitch += if settings.invert { -1.0 } else { 1.0 } * slow * input.dy / 650.0;

        p.view_yaw_delta = p.view_yaw_delta.clamp(-MAX_YAW as f32, MAX_YAW as f32);
        p.view_pitch = p.view_pitch.clamp(-FRAC_PI_2, FRAC_PI_2);
    } else {
        for s in &mut input.stick {
            *s = if *s < -1.0 { *s + 1.0 } else if *s <= 1.0 { 0.0 } else { *s - 1.0 };
        }

        input.stick[0] += 1.0 / 16.0 * input.dx;
        input.stick[1] += 1.0 / 16.0 * input.dy;

        p.gear_y = input.stick[0];
        p.gear_x -= input.stick[1];

        if !input.left {
            p.view_yaw_delta *= 0.875;
            p.view_pitch *= 0.875;
        }
    }

    p.zoom = (p.zoom + input.wheel as f32 * 0.125).clamp(0.0, 0.875);
    input.wheel = 0;
    p.zoom_eased += (p.zoom - p.zoom_eased) / 16.0;

    let axis = |plus: Control, minus: Control| input.held(plus) as i32 as f32 - input.held(minus) as i32 as f32;
    p.forward_back += 1.0 / 32.0 * axis(Control::Forward, Control::Back);
    p.left_right += axis(Control::Right, Control::Left);

    let keys = [(Control::Jump, flag::JUMP), (Control::Crouch, flag::CROUCH), (Control::Shift, flag::SHIFT), (Control::Use, flag::USE)];
    p.input_flags |= keys.iter().filter(|(k, _)| input.held(*k)).fold(0, |f, (_, b)| f | b);

    inventory_input(net, me, human, input, settings);

    let p = &mut net.players[me];

    for v in [&mut p.gear_x, &mut p.left_right, &mut p.gear_y, &mut p.forward_back] {
        *v = v.clamp(-1.0, 1.0);
    }
}
