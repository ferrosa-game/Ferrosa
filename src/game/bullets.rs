use bevy::{
    camera::visibility::NoFrustumCulling, image::ImageSampler, light::NotShadowCaster, mesh::{Indices, PrimitiveTopology}, prelude::*,
};
use ferrosa_protocol::clientbound::game::GameState;
use ferrosa_world::trace::line_intersect_level;

use super::{
    Level,
    effects::{empty_mesh, frame_across},
    netgame::{NetEvent, NetGame},
    network::NetTick,
    textures,
};
use crate::menu::Screen;

const MAX_BULLETS: usize = 0x4000;
const MAX_HOLES: usize = 0x4000;
const GRAVITY: f32 = 9.8 / 60.0 / 60.0;
const DRAG: [f32; 4] = [0.00456, 0.003916, 0.007952, 0.005832];

struct Bullet {
    kind: usize,
    time: i32,
    pos: Vec3,
    vel: Vec3,
}

struct Tracer {
    from: Vec3,
    to: Vec3,
    life: i32,
}

struct Hole {
    pos: Vec3,
    side: Vec3,
    up: Vec3,
    size: f32,
}

#[derive(Resource, Default)]
struct Shots {
    round: u32,
    bullets: Vec<Bullet>,
    tracers: Vec<Tracer>,
    holes: Vec<Hole>,
    holes_drawn: usize,
}

#[derive(Resource)]
struct ShotMeshes {
    tracers: Handle<Mesh>,
    holes: Handle<Mesh>,
}

pub struct BulletsPlugin;

impl Plugin for BulletsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Shots>()
            .add_systems(OnEnter(Screen::InGame), spawn_shot_meshes)
            .add_systems(FixedUpdate, fly_bullets.in_set(NetTick::Follow).run_if(in_state(Screen::InGame)))
            .add_systems(Update, draw_shots.run_if(in_state(Screen::InGame)).run_if(resource_exists::<ShotMeshes>))
            .add_systems(OnExit(Screen::InGame), |mut commands: Commands, mut shots: ResMut<Shots>| {
                *shots = Shots::default();
                commands.remove_resource::<ShotMeshes>();
            });
    }
}

fn spawn_shot_meshes(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut images: ResMut<Assets<Image>>) {
    let tracers = meshes.add(empty_mesh(PrimitiveTopology::LineList, 2));
    let holes = meshes.add(empty_mesh(PrimitiveTopology::TriangleList, 2).with_inserted_indices(Indices::U32(Vec::new())));

    let mut spot = textures::particle().into_image();
    spot.sampler = ImageSampler::linear();

    let line = StandardMaterial { base_color: Color::WHITE, unlit: true, alpha_mode: AlphaMode::Blend, ..default() };
    let hole = StandardMaterial { base_color: Color::BLACK, base_color_texture: Some(images.add(spot)), unlit: true, alpha_mode: AlphaMode::Blend, cull_mode: None, ..default() };

    commands.spawn((Mesh3d(tracers.clone()), MeshMaterial3d(materials.add(line)), NoFrustumCulling, DespawnOnExit(Screen::InGame), NotShadowCaster));
    commands.spawn((Mesh3d(holes.clone()), MeshMaterial3d(materials.add(hole)), NoFrustumCulling, DespawnOnExit(Screen::InGame), NotShadowCaster));
    commands.insert_resource(ShotMeshes { tracers, holes });
}

fn fly_bullets(net: Res<NetGame>, level: Option<Res<Level>>, mut shots: ResMut<Shots>, mut happenings: MessageReader<NetEvent>) {
    if shots.round != net.round {
        *shots = Shots { round: net.round, ..default() };
    }

    let fresh: Vec<NetEvent> = happenings.read().cloned().collect();
    if matches!(net.game_state, GameState::Restarting | GameState::Paused) {
        return;
    }

    let Some(level) = level else { return };
    let map = &level.map;
    let Shots { bullets, tracers, holes, .. } = &mut *shots;

    for b in bullets.iter_mut() {
        let from = b.pos;
        b.vel.y -= GRAVITY;

        let speed = b.vel.length();
        b.vel -= b.vel.normalize_or_zero() * speed * speed * DRAG[b.kind];
        b.pos += b.vel;

        if let Some(hit) = line_intersect_level(&map.ground, &map.level.area, &map.level.meshes, from, b.pos).filter(|h| h.hit.fraction < 1.0) {
            b.pos = hit.hit.pos;
            b.time = 0;
        }

        tracers.push(Tracer { from, to: b.pos, life: 3 });
    }

    for happening in fresh {
        match happening {
            NetEvent::Gunshot { bullet, pos, vel, .. } if bullets.len() < MAX_BULLETS => {
                bullets.push(Bullet { kind: usize::try_from(bullet).unwrap_or(0).min(DRAG.len() - 1), time: 600, pos, vel });
            }
            NetEvent::BulletHit { hit: 0, pos, normal } if holes.len() < MAX_HOLES => {
                let n = normal.normalize_or_zero();
                let (side, up) = frame_across(n);
                let size = (rand::random::<u32>() & 0x1f) as f32 / 1024.0 + 1.0 / 32.0;

                holes.push(Hole { pos: pos + n * (1.0 / 64.0), side, up, size });
            }
            _ => {}
        }
    }

    bullets.retain_mut(|b| {
        b.time -= 1;
        b.time > 0
    });
    tracers.retain_mut(|t| {
        t.life -= 1;
        t.life > 0
    });
}

fn draw_shots(mut shots: ResMut<Shots>, handles: Res<ShotMeshes>, mut meshes: ResMut<Assets<Mesh>>) {
    if let Some(mut mesh) = meshes.get_mut(&handles.tracers) {
        let mut positions = Vec::with_capacity(shots.tracers.len() * 2);
        let mut colours = Vec::with_capacity(shots.tracers.len() * 2);

        for t in &shots.tracers {
            let start = ((t.life - 1) as f32 * 0.25).clamp(0.0, 0.5);
            let end = (t.life as f32 * 0.5).min(1.0) * 0.5;

            positions.extend([t.from.to_array(), t.to.to_array()]);
            colours.extend([[1.0, 1.0, 1.0, start], [1.0, 1.0, 1.0, end]]);
        }

        if positions.is_empty() {
            positions = vec![[0.0; 3]; 2];
            colours = vec![[0.0; 4]; 2];
        }

        let count = positions.len();
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0f32]; count]);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32; 2]; count]);
    }

    if shots.holes_drawn == shots.holes.len() {
        return;
    }

    let Some(mut mesh) = meshes.get_mut(&handles.holes) else { return };
    let count = shots.holes.len();
    let (mut positions, mut normals, mut uvs, mut indices) = (Vec::with_capacity(count * 4), Vec::with_capacity(count * 4), Vec::with_capacity(count * 4), Vec::with_capacity(count * 6));

    for (k, h) in shots.holes.iter().enumerate() {
        let s = h.size;
        let n = h.side.cross(h.up).to_array();

        for (a, b, u, v) in [(-s, s, 0.0, 1.0), (s, s, 1.0, 1.0), (s, -s, 1.0, 0.0), (-s, -s, 0.0, 0.0)] {
            positions.push((h.pos + h.side * a + h.up * b).to_array());
            normals.push(n);
            uvs.push([u, v]);
        }

        let base = (k * 4) as u32;
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0f32; 4]; count * 4]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    shots.holes_drawn = count;
}
