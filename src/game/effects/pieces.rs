use bevy::{light::NotShadowCaster, prelude::*};
use ferrosa_protocol::clientbound::game::GameState;

use super::{DRAG, EffectAssets, GRAVITY, Particles, RESTING_LIFT, spin_axes, trace};
use crate::{
    game::{
        Level,
        level_render::AsphaltMaterial,
        netgame::{NetEvent, NetGame},
    },
    menu::Screen,
};

const MAX_PIECES: usize = 0x4000;
const FLOOR: f32 = 0.7;
const STILL: f32 = 1.0 / 120.0;
const CLINK: f32 = 1.0 / 60.0;
const MIN_SPIN: f32 = 1.0 / 65536.0;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum PieceKind {
    Case,
    Chip,
}

#[derive(Component)]
pub(super) struct Piece {
    kind: PieceKind,
    pos: Vec3,
    vel: Vec3,
    axes: [Vec3; 3],
    spin: Vec3,
    was: (Vec3, [Vec3; 3]),
}

fn piece_bounce(vel: Vec3, n: Vec3) -> Vec3 {
    let d = vel.dot(n);
    let mut along = vel - n * d;

    if along.length() > d.abs() * 0.7 {
        along = along.normalize_or_zero() * d.abs() * 0.7;
    }

    (vel - n * d * 1.25 - along) * 15.0 / 16.0
}

fn piece_transform(pos: Vec3, axes: &[Vec3; 3]) -> Transform {
    Transform::from_translation(pos).with_rotation(Quat::from_mat3(&Mat3::from_cols(axes[0], axes[1], axes[2])).normalize())
}

pub(super) fn spawn_piece(
    commands: &mut Commands,
    effect_assets: &EffectAssets,
    asphalt: Option<&AsphaltMaterial>,
    particles: &mut Particles,
    kind: PieceKind,
    pos: Vec3,
    vel: Vec3,
    axes: [Vec3; 3],
) {
    if particles.pieces >= MAX_PIECES {
        return;
    }

    let turn = (rand::random_range(-1.0f32..1.0) * 4.0 + 12.0) / 60.0;
    let back = -(rand::random_range(-1.0f32..1.0) * 4.0 + 12.0) / 60.0;
    let spin = axes[1] * turn + axes[0] * back;
    let (mesh, material) = match kind {
        PieceKind::Case => effect_assets.case.clone(),
        PieceKind::Chip => (effect_assets.chip.0.clone(), asphalt.map_or(effect_assets.chip.1.clone(), |a| a.0.clone())),
    };

    particles.pieces += 1;
    commands.spawn((
        Piece { kind, pos, vel, axes, spin, was: (pos, axes) },
        Mesh3d(mesh),
        MeshMaterial3d(material),
        piece_transform(pos, &axes),
        NotShadowCaster,
        DespawnOnExit(Screen::InGame),
    ));
}

pub(super) fn simulate_pieces(mut net: ResMut<NetGame>, level: Option<Res<Level>>, mut pieces: Query<&mut Piece>) {
    if matches!(net.game_state, GameState::Restarting | GameState::Paused) {
        return;
    }

    let Some(level) = level else { return };
    let map = &level.map;

    for mut piece in &mut pieces {
        let piece = &mut *piece;
        piece.was = (piece.pos, piece.axes);

        if piece.vel == Vec3::ZERO && piece.spin == Vec3::ZERO {
            continue;
        }

        let turn = piece.spin.length();

        if turn > MIN_SPIN {
            spin_axes(&mut piece.axes, piece.spin / turn, turn);
        }

        piece.vel = Vec3::new(piece.vel.x, piece.vel.y - GRAVITY, piece.vel.z) * DRAG;

        let old = piece.pos;
        piece.pos += piece.vel;

        if let Some((at, n)) = trace(map, old, piece.pos) {
            let hit = piece.vel;
            piece.pos = at + n * RESTING_LIFT;
            piece.vel = piece_bounce(piece.vel, n);

            let into = n.dot(hit);

            if piece.kind == PieceKind::Case && into < -CLINK {
                let pitch = (rand::random::<u32>() & 0x7f) as f32 / 1024.0 + 1.0;
                net.happenings.push(NetEvent::ShellHit { pos: piece.pos, volume: -into * 0.125 * 0.5 * 60.0, pitch });
            }

            let flat = piece.axes[0].cross(Vec3::Y).normalize_or_zero();
            piece.axes = [Vec3::Y.cross(flat).normalize_or_zero(), Vec3::Y, flat];

            if n.y > FLOOR && piece.vel.length() < STILL {
                piece.vel = Vec3::ZERO;
                piece.spin = Vec3::ZERO;
            }
        }
    }
}

pub(super) fn place_pieces(time: Res<Time<Fixed>>, mut pieces: Query<(&Piece, &mut Transform)>) {
    let a = time.overstep_fraction();

    for (piece, mut transform) in &mut pieces {
        let (from, to) = (piece_transform(piece.was.0, &piece.was.1), piece_transform(piece.pos, &piece.axes));
        let place = Transform::from_translation(from.translation.lerp(to.translation, a)).with_rotation(from.rotation.slerp(to.rotation, a));

        if *transform != place {
            *transform = place;
        }
    }
}
