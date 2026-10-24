use bevy::prelude::*;
use ferrosa_protocol::clientbound::game::GameState;

use super::{align, anchored, colour, own, rect, shown, text};
use crate::{fonts::GameFonts, game::netgame::NetGame};

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
pub(super) struct GearNotch(usize);

#[derive(Component)]
pub(super) struct GearText;

pub(super) fn spawn(commands: &mut Commands, centre: Entity, bottom_left: Entity, fonts: &GameFonts) {
    commands.spawn((rect(align::TOP_LEFT, Vec2::new(384.0, 288.0), Vec2::new(256.0, 32.0)), ProgressBar, BackgroundColor(colour([1.0, 1.0, 1.0, 0.125])), ChildOf(centre)));
    commands.spawn((rect(align::TOP_LEFT, Vec2::new(384.0, 288.0), Vec2::new(0.0, 32.0)), ProgressFill, BackgroundColor(colour([1.0, 1.0, 1.0, 0.875])), ChildOf(centre)));

    let track = Node { border: UiRect::all(px(1)), ..rect(align::BOTTOM, Vec2::new(24.0, 560.0), Vec2::new(10.0, 129.0)) };
    let bar = commands.spawn((track, StaminaBar, BackgroundColor(colour([0.0, 0.0, 0.0, 0.75])), BorderColor::all(colour([1.0, 1.0, 1.0, 0.75])), ChildOf(bottom_left))).id();
    let inside = Node { position_type: PositionType::Absolute, left: px(0), bottom: px(0), width: percent(100), ..default() };

    commands.spawn((Node { height: percent(0), ..inside.clone() }, StaminaFill, BackgroundColor(Color::NONE), ChildOf(bar)));
    commands.spawn((Node { height: px(2), ..inside }, StaminaMax, BackgroundColor(colour([1.0, 1.0, 1.0, 0.875])), ChildOf(bar)));

    for k in 0..3 {
        let notch = Node { border: UiRect::all(px(1)), ..rect(align::BOTTOM, Vec2::new(38.0, 560.0 - 12.0 * k as f32), Vec2::new(10.0, 10.0)) };

        commands.spawn((notch, GearNotch(k), BackgroundColor(Color::NONE), BorderColor::all(Color::NONE), ChildOf(bottom_left)));
    }

    let (node, shift) = anchored(align::BOTTOM_CENTRE, Vec2::new(36.0, 574.0));
    commands.spawn((node, shift, GearText, text("", &fonts.rockwell, 12.0, colour([1.0, 1.0, 1.0, 0.75])), ChildOf(bottom_left)));
}

fn stamina_colour(stamina: i32) -> Color {
    let t = (stamina as f32 / 127.0).clamp(0.0, 1.0);
    let dim = 1.0 - 0.25 * t;

    colour([(2.0 - 2.0 * t).min(1.0) * dim, (2.0 * t).min(1.0) * dim, 0.0, 0.875])
}

pub(super) fn show_status(
    net: Res<NetGame>,
    mut bars: ParamSet<(
        Query<(Has<ProgressBar>, &mut Visibility), Or<(With<ProgressBar>, With<StaminaBar>)>>,
        Query<(&mut Node, &mut Visibility), With<ProgressFill>>,
        Query<(&mut Node, &mut Visibility, &mut BackgroundColor), With<StaminaFill>>,
        Query<(&mut Node, &mut Visibility), With<StaminaMax>>,
        Query<(&GearNotch, &mut BackgroundColor, &mut BorderColor, &mut Visibility)>,
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

    for (mut node, mut v, mut fill) in &mut bars.p2() {
        shown(&mut v, playing);
        node.height = percent((own.stamina as f32 / 127.0).min(1.0) * 100.0);
        fill.0 = stamina_colour(own.stamina);
    }

    let cap = own.max_stamina.min(127);

    for (mut node, mut v) in &mut bars.p3() {
        shown(&mut v, playing && cap < 127);
        node.bottom = percent(cap as f32 / 127.0 * 100.0);
    }

    for (&GearNotch(k), mut fill, mut edge, mut v) in &mut bars.p4() {
        let on = k as u32 == p.zoom_level;

        shown(&mut v, playing && h.vehicle < 0);

        fill.0 = colour(if on { [1.0, 1.0, 1.0, 0.875] } else { [0.0, 0.0, 0.0, 0.75] });
        *edge = BorderColor::all(colour([1.0, 1.0, 1.0, if on { 0.75 } else { 0.25 }]));
    }

    for (mut t, mut v) in &mut bars.p5() {
        let gear = if playing && h.vehicle < 0 {
            Some(match p.zoom_level {
                0 => "Run",
                1 => "Walk",
                2 => "Aim",
                _ => "How?"
            })
        } else {
            None
        };

        shown(&mut v, gear.is_some());

        if let Some(gear) = gear && t.0 != *gear {
            t.0 = gear.to_string();
        }
    }
}
