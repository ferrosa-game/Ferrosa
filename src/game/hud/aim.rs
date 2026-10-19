use std::f32::consts::PI;

use bevy::prelude::*;
use ferrosa_protocol::clientbound::game::GameState;

use super::{align, colour, line, own, rect, shown};
use crate::game::{
    controls::{InputState, flag},
    items::ItemTypes,
    netgame::NetGame,
};

const AIM_PIVOT: Vec2 = Vec2::new(608.0, 352.0);
const AIM_NEEDLE: f32 = 64.0;

#[derive(Component)]
pub(super) struct Dot;

#[derive(Component)]
pub(super) struct GunTick(usize);

#[derive(Component)]
pub(super) struct AimLine(usize);

pub(super) fn spawn(commands: &mut Commands, centre: Entity) {
    commands.spawn((rect(align::CENTRE, Vec2::new(512.0, 288.0), Vec2::splat(4.0)), Dot, BackgroundColor(colour([1.0, 1.0, 1.0, 0.5])), ChildOf(centre)));

    for side in 0..4 {
        commands.spawn((Node::default(), GunTick(side), BackgroundColor(colour([1.0, 1.0, 1.0, 0.5])), ChildOf(centre)));
    }

    for (k, shade) in [[1.0, 1.0, 1.0, 0.5], [1.0, 1.0, 1.0, 0.5], [1.0, 1.0, 0.0, 0.75], [1.0, 1.0, 0.0, 0.5], [1.0, 1.0, 0.0, 0.5]].into_iter().enumerate() {
        commands.spawn((Node::default(), AimLine(k), BackgroundColor(colour(shade)), Visibility::Hidden, ChildOf(centre)));
    }
}

pub(super) fn show_crosshair(
    net: Res<NetGame>,
    types: Res<ItemTypes>,
    mut dot: Query<&mut Visibility, (With<Dot>, Without<GunTick>)>,
    mut ticks: Query<(&GunTick, &mut Node, &mut UiTransform, &mut Visibility)>,
) {
    let ours = own(&net).filter(|_| net.game_state != GameState::Restarting);
    let on_foot = ours.is_some_and(|(_, h, _)| h.vehicle < 0);

    dot.iter_mut().for_each(|mut v| shown(&mut v, on_foot));

    let gun = ours.filter(|(p, _, _)| p.input_flags & flag::SHIFT == 0).and_then(|(p, _, _)| {
        let held = p.display_inventory[0].first().map(|&c| (c & 0xff) as usize)?;
        types.0.get(held).filter(|t| t.is_gun).map(|_| 8.0 / (1.0 - p.zoom_eased))
    });

    for (&GunTick(side), mut node, mut turn, mut v) in &mut ticks {
        shown(&mut v, gun.is_some());

        let Some(gap) = gun else { continue };
        let centre = Vec2::new(512.0, 288.0);
        let toward = [Vec2::NEG_Y, Vec2::Y, Vec2::NEG_X, Vec2::X][side];
        let (n, t) = line(centre + toward * gap, centre + toward * gap * 2.0, 1.0, false);

        *node = n;
        *turn = t;
    }
}

pub(super) fn show_aim(net: Res<NetGame>, input: Res<InputState>, mut lines: Query<(&AimLine, &mut Node, &mut UiTransform, &mut Visibility)>) {
    let pitch = own(&net).filter(|_| net.game_state != GameState::Restarting && input.aiming()).map(|(p, _, _)| p.free_look_pitch);
    let needle = |a: f32| AIM_PIVOT + Vec2::new(-a.cos() * AIM_NEEDLE * 0.5, a.sin() * AIM_NEEDLE);
    let tip = |pitch: f32, scale: f32| needle(15.0 * pitch * scale);

    for (&AimLine(k), mut node, mut turn, mut v) in &mut lines {
        shown(&mut v, pitch.is_some());

        let Some(pitch) = pitch else { continue };
        let (a, b) = match k {
            0 => (AIM_PIVOT, needle(-PI / 8.0)),
            1 => (AIM_PIVOT, needle(PI / 8.0)),
            2 => (AIM_PIVOT, tip(pitch, 1.0)),
            3 => (tip(pitch, 1.0), tip(pitch, 15.0 / 16.0)),
            _ => (tip(pitch, 15.0 / 16.0), tip(pitch, 0.875)),
        };

        (*node, *turn) = line(a, b, 1.0, false);
    }
}
