use std::f32::consts::{PI, TAU};

use bevy::prelude::*;
use ferrosa_protocol::clientbound::game::GameState;

use super::{align, anchored, colour, line, own, rect, shown, text};
use crate::game::netgame::NetGame;

const GEAR_LABELS: [(&str, f32, f32); 6] = [("1", 818.0, 420.0), ("2", 818.0, 476.0), ("3", 850.0, 420.0), ("4", 850.0, 476.0), ("A", 882.0, 420.0), ("R", 882.0, 476.0)];

#[derive(Component)]
pub(super) struct CarGauges;

#[derive(Component)]
pub(super) struct Needle(usize);

#[derive(Component)]
pub(super) struct GearKnob;

#[derive(Component)]
pub(super) struct Collective;

#[derive(Component)]
pub(super) struct CollectiveLever;

pub(super) fn spawn(commands: &mut Commands, screen: Entity, font: &Handle<Font>) {
    let shade = BackgroundColor(colour([0.0, 0.0, 0.0, 0.75]));
    let car = commands.spawn((Node { position_type: PositionType::Absolute, ..default() }, CarGauges, Pickable::IGNORE, Visibility::Hidden, ChildOf(screen))).id();

    for (k, (centre, radius)) in [(Vec2::new(548.0, 480.0), 36.0), (Vec2::new(480.0, 480.0), 32.0)].into_iter().enumerate() {
        let dial = Node { border: UiRect::all(px(2)), border_radius: BorderRadius::all(percent(50)), ..rect(align::CENTRE, centre, Vec2::splat(radius * 2.0)) };
        commands.spawn((dial, BackgroundColor(colour([0.0, 0.0, 0.0, 0.25])), BorderColor::all(Color::BLACK), ChildOf(car)));

        let (node, turn) = line(centre, centre + Vec2::new(0.0, -radius), 1.5, false);
        commands.spawn((node, turn, Needle(k), BackgroundColor(colour([0.75, 0.75, 0.75, 1.0])), ChildOf(car)));
    }

    for x in [818.0, 850.0, 882.0] {
        commands.spawn((rect(align::CENTRE, Vec2::new(x, 448.0), Vec2::new(8.0, 68.0)), shade, ChildOf(car)));
    }

    commands.spawn((rect(align::CENTRE, Vec2::new(850.0, 448.0), Vec2::new(72.0, 8.0)), shade, ChildOf(car)));

    for (label, x, y) in GEAR_LABELS {
        let (node, shift) = anchored(align::CENTRE, Vec2::new(x, y));
        commands.spawn((node, shift, text(label, font, 16.0, colour([1.0, 1.0, 1.0, 0.75])), ChildOf(car)));
    }

    let knob = Node { border_radius: BorderRadius::all(percent(50)), ..rect(align::CENTRE, Vec2::new(818.0, 448.0), Vec2::splat(24.0)) };
    commands.spawn((knob, GearKnob, BackgroundColor(colour([1.0, 1.0, 1.0, 0.75])), ChildOf(car)));

    commands.spawn((rect(align::CENTRE, Vec2::new(818.0, 448.0), Vec2::new(16.0, 80.0)), Collective, BackgroundColor(colour([0.0, 0.0, 0.0, 0.5])), Visibility::Hidden, ChildOf(screen)));
    commands.spawn((rect(align::CENTRE, Vec2::new(818.0, 448.0), Vec2::splat(16.0)), CollectiveLever, BackgroundColor(colour([1.0, 1.0, 1.0, 0.5])), Visibility::Hidden, ChildOf(screen)));
}

pub(super) fn show_gauges(
    net: Res<NetGame>,
    mut car: Query<&mut Visibility, (With<CarGauges>, Without<Collective>, Without<CollectiveLever>)>,
    mut needles: Query<(&Needle, &mut Node, &mut UiTransform), Without<GearKnob>>,
    mut knob: Query<&mut Node, (With<GearKnob>, Without<Needle>, Without<CollectiveLever>)>,
    mut collective: Query<&mut Visibility, (With<Collective>, Without<CarGauges>, Without<CollectiveLever>)>,
    mut lever: Query<(&mut Node, &mut Visibility), (With<CollectiveLever>, Without<Needle>, Without<GearKnob>)>,
) {
    let driving = own(&net).filter(|(_, h, _)| net.game_state != GameState::Restarting && h.vehicle >= 0 && h.vehicle_seat == 0);
    let input = driving.map_or(0, |(p, _, _)| p.last_input_type);

    car.iter_mut().for_each(|mut v| shown(&mut v, input == 2));
    collective.iter_mut().for_each(|mut v| shown(&mut v, input == 3));

    for (mut node, mut v) in &mut lever {
        shown(&mut v, input == 3);

        if let Some((p, _, _)) = driving {
            node.top = px(448.0 - 32.0 * p.forward_back - 8.0);
        }
    }

    let Some((p, h, _)) = driving.filter(|_| input == 2) else { return };
    let Some(v) = net.vehicles.get(h.vehicle as usize) else { return };

    let ticks = (v.track.tick - v.track.prev_tick).max(1) as f32;
    let speed = (v.track.pos - v.track.prev_pos).length() / ticks;
    let mph = speed * 60.0 * 60.0 * 60.0 * 0.621_371_2 / 1000.0 / 180.0;
    let turns = [mph * TAU, v.engine_rpm as f32 / 5000.0 * PI];

    for (&Needle(k), mut node, mut turn) in &mut needles {
        let (centre, radius) = [(Vec2::new(548.0, 480.0), 36.0), (Vec2::new(480.0, 480.0), 32.0)][k];
        let angle = turns[k] - PI * 0.75;
        let (n, t) = line(centre, centre + Vec2::new(angle.sin(), -angle.cos()) * radius, 1.5, false);

        *node = n;
        *turn = t;
    }

    for mut node in &mut knob {
        node.left = px(818.0 + 32.0 * p.gear_x - 12.0);
        node.top = px(448.0 + 32.0 * p.gear_y - 12.0);
    }
}
