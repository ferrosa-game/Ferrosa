use bevy::prelude::*;
use ferrosa_protocol::{GameMode, clientbound::game::GameState};

use super::{align, anchored, colour, in_hand, is_phone, own, text};
use crate::{fonts::GameFonts, game::netgame::NetGame};

const PHONE_LINES: [&str; 3] = ["1111 - Goldmen", "2222 - Monsota", "3333 - OXS"];
const WORLD_PHONE_LINES: [&str; 3] = ["4444 - Nexaco", "5555 - Pentacom", "6666 - Prodocon"];

#[derive(Component)]
pub(super) struct PhoneHelp;

pub(super) fn spawn(commands: &mut Commands, centre: Entity) {
    let (node, shift) = anchored(align::CENTRE_X, Vec2::new(512.0, 384.0));
    commands.spawn((Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..node }, shift, PhoneHelp, ChildOf(centre)));
}

fn phone_help(net: &NetGame, item: usize) -> Vec<(String, f32, Color)> {
    let (status, number, _) = net.items[item].phone;
    let white = Color::WHITE;
    let line = |s: &str| (s.to_string(), 16.0, white);

    let mut lines = match status {
        0 => vec![line("Left-click to dial")],
        1 => {
            let world = if net.sync.game_type == GameMode::World { &WORLD_PHONE_LINES[..] } else { &[] };
            [&["Use Numpad or number keys"][..], &PHONE_LINES, world].concat().into_iter().map(line).collect()
        }
        4 => vec![line("Busy")],
        2 => vec![line(if number > 0 { "Left-click to answer" } else { "Left-click to hang up" })],
        3 => vec![("In Use".to_string(), 24.0, colour([0.0, 1.0, 0.0, 1.0]))],
        _ => vec![],
    };

    if matches!(status, 1 | 2) && number > 0 {
        lines.push((format!("Number: {number}"), 24.0, white));
    }

    lines
}

pub(super) fn show_phone_help(
    mut commands: Commands,
    net: Res<NetGame>,
    fonts: Res<GameFonts>,
    help: Single<(Entity, Option<&Children>), With<PhoneHelp>>,
    mut last: Local<Vec<(String, f32, Color)>>,
) {
    let lines = own(&net)
        .filter(|_| net.game_state != GameState::Restarting)
        .and_then(|(_, h, _)| in_hand(&net, h, 0))
        .filter(|&(_, kind)| is_phone(kind))
        .map_or_else(Vec::new, |(item, _)| phone_help(&net, item));

    if *last == lines {
        return;
    }

    let (help, children) = *help;
    children.into_iter().flatten().for_each(|&c| commands.entity(c).despawn());

    for (s, size, colour) in &lines {
        commands.spawn((text(s.clone(), &fonts.rockwell, *size, *colour), ChildOf(help)));
    }

    *last = lines;
}
