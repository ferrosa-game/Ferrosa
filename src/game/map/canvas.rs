use bevy::prelude::*;
use ferrosa_protocol::{GameMode, Team};

use super::{
    BANK_LABEL, Canvas, DROP_COLOUR, MARKER_COLOUR, MARKER_EDGE, MARKER_SIZE, MINI_SIZE, MapPicture, MapView, RING_WIDTH, SPAN, SPOT_COLOUR, Viewport, fit_zoom, player_on_map,
};
use crate::{
    fonts::GameFonts,
    game::{
        Level,
        hud::{align, anchored, colour, rect, text},
        netgame::NetGame,
    },
    names::{ROUND_CORPORATIONS, TEAM_COLOURS},
};

const MINI_WORLD_SPAN: f32 = 256.0;
const BASE_LABEL: [f32; 4] = [0.95, 0.95, 0.95, 1.0];
const STREET_LABEL: [f32; 4] = [1.0, 1.0, 1.0, 0.85];
const LABEL: f32 = 6.0;
const LABEL_SIZES: (f32, f32) = (10.0, 16.0);
const BANK_SIGN: f32 = 9.0;
const BANK_SIZES: (f32, f32) = (12.0, 22.0);
const BASE: i32 = 1;
const BANK: i32 = 6;
const DROP_MISSION: i32 = 2;
const SIDE_STREETS: i32 = 0x140;
const AVENUE_START: i32 = 0x100;
const AVENUE_BLOCK: i32 = 0x38;
const GRID: f32 = 4.0;
const MISSION_SPOTS: [Vec2; 8] = [
    Vec2::new(1718.0, 1262.0),
    Vec2::new(1778.0, 1006.0),
    Vec2::new(1470.0, 1494.0),
    Vec2::new(1282.0, 1534.0),
    Vec2::new(1674.0, 1510.0),
    Vec2::new(1346.0, 1298.0),
    Vec2::new(1606.0, 1166.0),
    Vec2::new(1546.0, 1278.0),
];

#[derive(Component)]
pub(super) struct PlayerPointer;

#[derive(Clone, PartialEq)]
pub(super) struct Mark {
    at: Vec2,
    radius: f32,
    colour: [f32; 4],
}

fn marks(net: &NetGame, picture: &MapPicture, level: &Level) -> Vec<Mark> {
    let mut out = Vec::new();
    let me = net.me().map(|p| p.team);
    let team_colour = |i: usize| {
        let [r, g, b] = TEAM_COLOURS[i];
        [r, g, b, 1.0]
    };
    let base = |i: usize| level.map.level.bases.get(i).map(|b| picture.on_map(Vec2::new(b.interior_min.x + b.interior_max.x, b.interior_min.z + b.interior_max.z) * 0.5));
    let mine = |i: usize| me == Team::CORPORATIONS.get(i).copied();

    match net.sync.game_type {
        GameMode::World => {
            for i in 0..Team::CORPORATIONS.len() {
                let Some(at) = base(i) else { continue };

                out.push(Mark { at, radius: 16.0, colour: team_colour(i) });

                if mine(i) {
                    out.push(Mark { at, radius: 8.0, colour: team_colour(i) });
                }

                if net.players.iter().any(|p| p.active && p.team == Team::CORPORATIONS[i]) {
                    out.push(Mark { at, radius: 12.0, colour: team_colour(i) });
                }
            }
        }
        mode @ (GameMode::Round | GameMode::Eliminator) => {
            for i in 0..ROUND_CORPORATIONS.len() {
                let Some(at) = base(i) else { continue };

                out.push(Mark { at, radius: 16.0, colour: if mode == GameMode::Round { team_colour(i) } else { [1.0; 4] } });

                if mine(i) {
                    out.push(Mark { at, radius: 14.0, colour: team_colour(i) });
                }
            }
        }
        _ => {}
    }

    if net.sync.game_type == GameMode::Round
        && let Some(missions) = me.and_then(|t| Team::CORPORATIONS.iter().position(|&c| c == t)).and_then(|t| net.missions.get(t))
    {
        for m in missions.iter().filter(|m| m.active && m.kind == DROP_MISSION) {
            if let Some(&spot) = usize::try_from(m.location).ok().and_then(|k| MISSION_SPOTS.get(k)) {
                out.push(Mark { at: picture.on_map(spot), radius: 8.0, colour: DROP_COLOUR });
            }
        }
    }

    if net.sync.game_type == GameMode::Eliminator {
        out.extend(MISSION_SPOTS.iter().map(|&spot| Mark { at: picture.on_map(spot), radius: 8.0, colour: SPOT_COLOUR }));
    }

    out
}

fn label_size(base: f32, zoom: f32, (low, high): (f32, f32)) -> f32 {
    (base * zoom).clamp(low, high)
}

fn city_labels(commands: &mut Commands, canvas: Entity, picture: &MapPicture, level: &Level, font: &Handle<Font>, zoom: f32) {
    let streets = &level.map.streets;
    let mut label = |s: String, at: Vec2, size: f32, shade: [f32; 4]| {
        let (node, shift) = anchored(align::CENTRE, at * zoom);
        commands.spawn((node, shift, text(s, font, size, colour(shade)), TextShadow { offset: Vec2::splat(1.0), color: Color::BLACK }, ChildOf(canvas)));
    };

    for s in &streets.streets {
        let (Some(a), Some(b)) = (streets.intersections.get(s.intersections[0]), streets.intersections.get(s.intersections[1])) else { continue };
        let shown = match s.north_south {
            0 => a.streets[2] == -1,
            1 => (a.pos.z - SIDE_STREETS) as u32 <= 3,
            _ => true,
        };

        if !shown {
            continue;
        }

        let middle = Vec2::new((a.pos.x + b.pos.x) as f32, (a.pos.z + b.pos.z) as f32) * 0.5 * GRID;
        let stagger = if s.north_south == 1 { (((a.pos.x - AVENUE_START) / AVENUE_BLOCK) & 1) as f32 * 16.0 } else { 0.0 };

        label(s.name.to_string(), picture.on_map(middle) + Vec2::Y * stagger, label_size(LABEL, zoom, LABEL_SIZES), STREET_LABEL);
    }

    for b in &level.map.level.buildings {
        let at = picture.on_map(Vec2::new(b.interior_min.x + b.interior_max.x, b.interior_min.z + b.interior_max.z) * 0.5);

        match b.kind {
            BASE => {
                let name = usize::try_from(b.team).ok().and_then(|t| Team::CORPORATION_NAMES.get(t)).map_or_else(|| format!("team {}", b.team + 1), |n| n.to_string());
                label(name, at, label_size(LABEL, zoom, LABEL_SIZES), BASE_LABEL);
            }
            BANK => label("$".to_string(), at, label_size(BANK_SIGN, zoom, BANK_SIZES), BANK_LABEL),
            _ => {}
        }
    }
}

fn canvas_zoom(kind: Viewport, view: &MapView, picture: &MapPicture) -> f32 {
    match kind {
        Viewport::Full => view.zoom,
        Viewport::Mini => MINI_SIZE / (MINI_WORLD_SPAN * picture.scale),
    }
}

fn marker(centre: Vec2) -> Node {
    Node { border: UiRect::all(px(MARKER_EDGE)), border_radius: BorderRadius::all(percent(50)), ..rect(align::CENTRE, centre, Vec2::splat(MARKER_SIZE)) }
}

pub(super) fn draw_canvases(
    mut commands: Commands,
    net: Option<Res<NetGame>>,
    picture: Res<MapPicture>,
    level: Res<Level>,
    view: Res<MapView>,
    fonts: Res<GameFonts>,
    mut canvases: Query<(Entity, &mut Canvas, &ChildOf)>,
    viewports: Query<&Viewport>,
) {
    let Some(net) = net else { return };
    let wanted = marks(&net, &picture, &level);

    for (e, mut canvas, ChildOf(parent)) in &mut canvases {
        let Ok(&kind) = viewports.get(*parent) else { continue };
        let zoom = canvas_zoom(kind, &view, &picture);

        if canvas.0.as_ref().is_some_and(|(z, m)| *z == zoom && *m == wanted) {
            continue;
        }

        commands.entity(e).despawn_children();
        commands.spawn((rect(align::TOP_LEFT, Vec2::ZERO, Vec2::splat(SPAN * zoom)), ImageNode::new(picture.image.clone()), Pickable::IGNORE, ChildOf(e)));

        if !picture.round {
            city_labels(&mut commands, e, &picture, &level, &fonts.rockwell, zoom);
        }

        for m in &wanted {
            let ring = Node { border: UiRect::all(px(RING_WIDTH)), border_radius: BorderRadius::all(percent(50)), ..rect(align::CENTRE, m.at * zoom, Vec2::splat(m.radius * 2.0 * zoom.min(fit_zoom() * 2.0))) };
            commands.spawn((ring, BorderColor::all(colour(m.colour)), Pickable::IGNORE, ChildOf(e)));
        }

        commands.spawn((marker(Vec2::ZERO), PlayerPointer, BackgroundColor(colour(MARKER_COLOUR)), BorderColor::all(Color::BLACK), Visibility::Hidden, Pickable::IGNORE, ChildOf(e)));

        canvas.0 = Some((zoom, wanted.clone()));
    }
}

pub(super) fn place_canvases(net: Option<Res<NetGame>>, picture: Res<MapPicture>, view: Res<MapView>, mut canvases: Query<(&mut Node, &ChildOf), With<Canvas>>, viewports: Query<&Viewport>) {
    let player = net.and_then(|n| player_on_map(&n, &picture));

    for (mut node, ChildOf(parent)) in &mut canvases {
        let Ok(&kind) = viewports.get(*parent) else { continue };
        let offset = match kind {
            Viewport::Full => view.offset,
            Viewport::Mini => {
                let zoom = canvas_zoom(kind, &view, &picture);
                Vec2::splat(MINI_SIZE * 0.5) - player.unwrap_or(Vec2::splat(SPAN * 0.5)) * zoom
            }
        };

        if node.left != px(offset.x) || node.top != px(offset.y) {
            node.left = px(offset.x);
            node.top = px(offset.y);
        }
    }
}

pub(super) fn place_pointers(net: Option<Res<NetGame>>, picture: Res<MapPicture>, canvases: Query<&Canvas>, mut pointers: Query<(&ChildOf, &mut Node, &mut Visibility), With<PlayerPointer>>) {
    let player = net.and_then(|n| player_on_map(&n, &picture));

    for (ChildOf(canvas), mut node, mut visibility) in &mut pointers {
        let (Some(at), Some(zoom)) = (player, canvases.get(*canvas).ok().and_then(|c| c.0.as_ref().map(|(z, _)| *z))) else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        let placed = marker(at * zoom);

        if node.left != placed.left || node.top != placed.top {
            *node = placed;
        }

        visibility.set_if_neq(Visibility::Inherited);
    }
}
