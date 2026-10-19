use bevy::prelude::*;
use ferrosa_protocol::clientbound::game::GameState;

use super::{align, anchored, colour, own, rect, shown, text};
use crate::{fonts::GameFonts, game::netgame::NetGame};

const GEARS: [&str; 3] = ["Run", "Walk", "Aim"];

#[derive(Component)]
pub(super) struct ProgressBar;

#[derive(Component)]
pub(super) struct ProgressFill;

#[derive(Component)]
pub(super) struct StaminaBar;

#[derive(Component)]
pub(super) struct StaminaFill;

#[derive(Component)]
pub(super) struct StaminaMax;

#[derive(Component)]
pub(super) struct GearText;

pub(super) fn spawn(commands: &mut Commands, centre: Entity, bottom_left: Entity, fonts: &GameFonts) {
    commands.spawn((rect(align::TOP_LEFT, Vec2::new(384.0, 288.0), Vec2::new(256.0, 32.0)), ProgressBar, BackgroundColor(colour([1.0, 1.0, 1.0, 0.125])), ChildOf(centre)));
    commands.spawn((rect(align::TOP_LEFT, Vec2::new(384.0, 288.0), Vec2::new(0.0, 32.0)), ProgressFill, BackgroundColor(colour([1.0, 1.0, 1.0, 0.875])), ChildOf(centre)));

    commands.spawn((rect(align::BOTTOM, Vec2::new(24.0, 540.0), Vec2::new(24.0, 127.0)), StaminaBar, BackgroundColor(colour([1.0, 1.0, 1.0, 0.125])), ChildOf(bottom_left)));
    commands.spawn((rect(align::BOTTOM, Vec2::new(24.0, 540.0), Vec2::new(24.0, 0.0)), StaminaFill, BackgroundColor(colour([1.0, 1.0, 1.0, 0.5])), ChildOf(bottom_left)));
    commands.spawn((rect(align::CENTRE_Y, Vec2::new(24.0, 540.0), Vec2::new(24.0, 8.0)), StaminaMax, BackgroundColor(colour([1.0, 1.0, 1.0, 0.875])), ChildOf(bottom_left)));

    let (node, shift) = anchored(align::BOTTOM, Vec2::new(64.0, 572.0));
    commands.spawn((node, shift, GearText, text("", &fonts.rockwell, 24.0, Color::WHITE), ChildOf(bottom_left)));
}

pub(super) fn show_status(
    net: Res<NetGame>,
    mut bars: ParamSet<(
        Query<(Has<ProgressBar>, &mut Visibility), Or<(With<ProgressBar>, With<StaminaBar>)>>,
        Query<(&mut Node, &mut Visibility), With<ProgressFill>>,
        Query<(&mut Node, &mut Visibility), With<StaminaFill>>,
        Query<(&mut Node, &mut Visibility), With<StaminaMax>>,
        Query<(&mut Text, &mut Visibility), With<GearText>>,
    )>,
) {
    let Some((p, h, own)) = own(&net) else { return };
    let playing = net.game_state != GameState::Restarting;
    let progress = playing && own.progress_bar > 0;

    for (progress_bar, mut v) in &mut bars.p0() {
        shown(&mut v, if progress_bar { progress } else { playing });
    }

    for (mut node, mut v) in &mut bars.p1() {
        shown(&mut v, progress);
        node.width = px(own.progress_bar as f32);
    }

    for (mut node, mut v) in &mut bars.p2() {
        shown(&mut v, playing);
        node.height = px(own.stamina as f32);
        node.top = px(540.0 - own.stamina as f32);
    }

    for (mut node, mut v) in &mut bars.p3() {
        shown(&mut v, playing);
        node.top = px(540.0 - own.max_stamina.min(0x7f) as f32 - 4.0);
    }

    for (mut t, mut v) in &mut bars.p4() {
        let gear = GEARS.get(p.zoom_level as usize).filter(|_| playing && h.vehicle < 0);

        shown(&mut v, gear.is_some());

        if let Some(gear) = gear
            && t.0 != *gear
        {
            t.0 = gear.to_string();
        }
    }
}
