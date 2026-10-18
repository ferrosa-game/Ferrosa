use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    gltf::GltfAssetLabel,
    image::ImageSampler,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use ferrosa_protocol::clientbound::game::GameState;
use ferrosa_world::{
    area::{MESH, TYPE_MASK},
    map::Map,
    trace::line_intersect_level,
};

use super::{
    Level, blocks,
    camera::GameCamera,
    level_render::{AsphaltMaterial, BlockPanes, centred_mesh},
    netgame::{NetEvent, NetGame},
    network::NetTick,
    textures,
    vehicles::VehicleTypes,
};
use crate::menu::Screen;

const MAX_PARTICLES: usize = 0x4000;
const MAX_PIECES: usize = 0x4000;
const FOREVER: i32 = 0xffff;
const GRAVITY: f32 = 0.002_722_222_3;
const DRAG: f32 = 0.9975;
const SMOKE_DRAG: f32 = 0.975;
const SMOKE_LIFE: i32 = 0x3c;
const PANE_LIFE: i32 = 0xea60;
const SETTLING: i32 = 0xe808;
const LIFT: f32 = 0.015625;
const RESTING_LIFT: f32 = 0.001_953_125;
const SHARD_SIZE: f32 = 0.0625;
const SHARD_GRID: usize = 9;
const UPRIGHT: f32 = 0.707;
const FLOOR: f32 = 0.7;
const STILL: f32 = 0.008_333_334;
const CLINK: f32 = 0.016_666_668;
const MIN_SPIN: f32 = 1.525_878_9e-5;
const RICOCHET: i32 = 0;
const BODY: i32 = 1;
const METAL: i32 = 2;
const GRAZE: i32 = 3;
const CASE_MODEL: &str = "models/items/case.glb";
const CHIP_MODEL: &str = "models/items/chip.glb";
const CASE_METALLIC: f32 = 0.5;
const CASE_ROUGHNESS: f32 = 0.125;
const SMOKE_COLOUR: [f32; 3] = [1.0, 1.0, 0.75];
const CHIP_COLOUR: Color = Color::srgb(0.3, 0.3, 0.3);
const BLOOD_SHADE: Color = Color::srgb(0.25, 0.0, 0.0);
const GLASS: Color = Color::srgba(0.6, 0.7, 0.75, 0.35);

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Blood,
    Smoke,
    Shard,
    Triangle,
}

#[derive(Clone, Copy)]
struct Particle {
    kind: Kind,
    life: i32,
    gravity: f32,
    drag: f32,
    size: f32,
    pos: Vec3,
    vel: Vec3,
    frame: [Vec3; 3],
    corner_vel: [Vec3; 3],
    rest: [f32; 3],
    spin_axis: Vec3,
    spin: f32,
}

impl Particle {
    fn new(kind: Kind, pos: Vec3, vel: Vec3) -> Self {
        Self { kind, life: FOREVER, gravity: GRAVITY, drag: DRAG, size: 0.0, pos, vel, frame: [Vec3::ZERO; 3], corner_vel: [Vec3::ZERO; 3], rest: [0.0; 3], spin_axis: Vec3::ZERO, spin: 0.0 }
    }
}

#[derive(Resource, Default)]
struct Particles {
    round: u32,
    list: Vec<Particle>,
    pieces: usize,
}

impl Particles {
    fn add(&mut self, p: Particle) {
        if self.list.len() < MAX_PARTICLES {
            self.list.push(p);
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum PieceKind {
    Case,
    Chip,
}

#[derive(Component)]
struct Piece {
    kind: PieceKind,
    pos: Vec3,
    vel: Vec3,
    axes: [Vec3; 3],
    spin: Vec3,
    was: (Vec3, [Vec3; 3]),
}

#[derive(Resource)]
struct EffectAssets {
    case: (Handle<Mesh>, Handle<StandardMaterial>),
    chip: (Handle<Mesh>, Handle<StandardMaterial>),
    spots: Handle<Mesh>,
    blood: Handle<Mesh>,
    glass: Handle<Mesh>,
}

pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Particles>()
            .add_systems(OnEnter(Screen::InGame), spawn_effect_assets)
            .add_systems(FixedUpdate, (emit_effects, simulate_effects).chain().in_set(NetTick::Follow).run_if(in_state(Screen::InGame)).run_if(resource_exists::<EffectAssets>))
            .add_systems(Update, (draw_particles, place_pieces).run_if(in_state(Screen::InGame)).run_if(resource_exists::<EffectAssets>))
            .add_systems(OnExit(Screen::InGame), |mut commands: Commands, mut particles: ResMut<Particles>| {
                *particles = Particles::default();
                commands.remove_resource::<EffectAssets>();
            });
    }
}

fn empty_mesh() -> Mesh {
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; 3])
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0f32]; 3])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32; 2]; 3])
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0f32; 4]; 3])
        .with_inserted_indices(Indices::U32(vec![0, 1, 2]))
}

fn spawn_effect_assets(mut commands: Commands, assets: Res<AssetServer>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut images: ResMut<Assets<Image>>) {
    let primitive = |path: &str| assets.load(GltfAssetLabel::Primitive { mesh: 0, primitive: 0 }.from_asset(path.to_string()));
    let case_texture = assets.load(GltfAssetLabel::Texture(0).from_asset(CASE_MODEL));
    let case_material = materials.add(StandardMaterial { base_color_texture: Some(case_texture), metallic: CASE_METALLIC, perceptual_roughness: CASE_ROUGHNESS, ..default() });
    let chip_material = materials.add(StandardMaterial { base_color: CHIP_COLOUR, perceptual_roughness: 0.9, ..default() });

    let mut spot = textures::particle().into_image();
    spot.sampler = ImageSampler::linear();

    let spots = meshes.add(empty_mesh());
    let blood = meshes.add(empty_mesh());
    let glass = meshes.add(empty_mesh());
    let spot_image = images.add(spot);
    let spot_material = StandardMaterial { base_color: Color::WHITE, base_color_texture: Some(spot_image.clone()), unlit: true, alpha_mode: AlphaMode::Blend, cull_mode: None, ..default() };
    let blood_material = StandardMaterial { base_color: BLOOD_SHADE, base_color_texture: Some(spot_image), perceptual_roughness: 1.0, alpha_mode: AlphaMode::Blend, cull_mode: None, ..default() };
    let glass_material = StandardMaterial { base_color: GLASS, alpha_mode: AlphaMode::Blend, perceptual_roughness: 0.05, reflectance: 1.0, double_sided: true, cull_mode: None, ..default() };

    commands.spawn((Mesh3d(spots.clone()), MeshMaterial3d(materials.add(spot_material)), NoFrustumCulling, NotShadowCaster, DespawnOnExit(Screen::InGame)));
    commands.spawn((Mesh3d(blood.clone()), MeshMaterial3d(materials.add(blood_material)), NoFrustumCulling, NotShadowCaster, DespawnOnExit(Screen::InGame)));
    commands.spawn((Mesh3d(glass.clone()), MeshMaterial3d(materials.add(glass_material)), NoFrustumCulling, NotShadowCaster, DespawnOnExit(Screen::InGame)));
    commands.insert_resource(EffectAssets { case: (primitive(CASE_MODEL), case_material), chip: (primitive(CHIP_MODEL), chip_material), spots, blood, glass });
}

fn random_vec3(scale: f32, base: f32) -> Vec3 {
    let r = || rand::random_range(-1.0f32..1.0);
    let d = Vec3::new(r(), r(), r()).normalize_or_zero();

    d * ((r() + 1.0) * scale * 0.5 + base)
}

fn byte() -> f32 {
    rand::random::<u8>() as f32
}

pub fn frame_across(n: Vec3) -> (Vec3, Vec3) {
    let axis = if n.y.abs() >= UPRIGHT { Vec3::X } else { Vec3::Y };
    let side = axis.cross(n).normalize_or_zero();

    (side, n.cross(side).normalize_or_zero())
}

fn trace(map: &Map, start: Vec3, end: Vec3) -> Option<(Vec3, Vec3)> {
    line_intersect_level(&map.ground, &map.level.area, &map.level.meshes, start, end).map(|h| (h.hit.pos, h.hit.normal))
}

fn bounce(vel: Vec3, n: Vec3, keep: f32) -> Vec3 {
    let d = vel.dot(n);
    let mut along = vel - n * d;
    let most = d.abs() * 0.7;

    if along.length() > most {
        along *= most / along.length();
    }

    (vel - n * d * 1.125 - along) * keep
}

fn piece_bounce(vel: Vec3, n: Vec3) -> Vec3 {
    let d = vel.dot(n);
    let mut along = vel - n * d;

    if along.length() > d.abs() * 0.7 {
        along = along.normalize_or_zero() * d.abs() * 0.7;
    }

    (vel - n * d * 1.25 - along) * 0.9375
}

fn spin_axes(axes: &mut [Vec3; 3], axis: Vec3, angle: f32) {
    let turn = Quat::from_axis_angle(axis, angle);

    axes.iter_mut().for_each(|a| *a = turn * *a);
}

fn piece_transform(pos: Vec3, axes: &[Vec3; 3]) -> Transform {
    Transform::from_translation(pos).with_rotation(Quat::from_mat3(&Mat3::from_cols(axes[0], axes[1], axes[2])).normalize())
}

fn shatter_window(particles: &mut Particles, c: [Vec3; 4], hit: Vec3, hit_vel: Vec3) {
    let n = (c[1] - c[0]).cross(c[2] - c[0]).normalize_or_zero();
    let (side, up) = frame_across(n);
    let steps = (SHARD_GRID - 1) as f32;
    let (left, right) = ((c[3] - c[0]) / steps, (c[2] - c[1]) / steps);
    let (mut start, mut end) = (c[0], c[1]);

    for _ in 0..SHARD_GRID {
        let step = (end - start) / steps;
        let mut p = start;

        for _ in 0..SHARD_GRID {
            let near = (1.0 - p.distance(hit) * 4.0).max(0.03125);
            let vel = (p - hit) * 0.03125 + random_vec3(0.008_333_334, 0.002_083_333_4) + hit_vel * near * 0.0625;
            let mut s = Particle::new(Kind::Shard, p, vel);

            s.size = SHARD_SIZE;
            s.frame = [side, up, n];
            s.spin_axis = Vec3::new(byte() * 0.001_953_125 - 0.25, byte() * 0.001_953_125 - 0.25, byte() * 0.001_953_125 - 0.25).normalize_or_zero();
            s.spin = (byte() * 0.001_953_125 - 0.25) * 0.5;
            particles.add(s);
            p += step;
        }

        start += left;
        end += right;
    }
}

fn break_pane(particles: &mut Particles, level: &mut Level, cell: IVec3, face: i32, hit: Vec3, hit_vel: Vec3) -> bool {
    let area = &level.map.level.area;
    let word = area.layer1(cell.x, cell.y, cell.z);

    if word & TYPE_MASK != MESH {
        return false;
    }

    let bs = area.block_size;
    let wall = level.map.level.meshes.get(word & 0xffff).and_then(|m| m.walls.get(face as usize));

    if let Some(w) = wall {
        let at = |c: Vec3| (c + cell.as_vec3()) * bs + area.origin;
        let c = [at(w[3]), at(w[2]), at(w[1]), at(w[0])];

        for k in 0..4 {
            let tip = if k == 2 { hit - Vec3::Y * 0.0625 } else { hit };
            let mut t = Particle::new(Kind::Triangle, Vec3::ZERO, Vec3::ZERO);

            t.frame = [c[k], c[(k + 1) & 3], tip];
            t.corner_vel = [Vec3::ZERO, Vec3::ZERO, hit_vel * 0.015625];
            t.life = PANE_LIFE;
            t.rest = [t.frame[0].distance(t.frame[1]), t.frame[1].distance(t.frame[2]), t.frame[2].distance(t.frame[0])];
            particles.add(t);
        }
    }

    let broken = word | 0x10000u32.checked_shl(face as u32).unwrap_or(0);
    level.map.level.area.set_layer1(cell.x, cell.y, cell.z, broken);

    true
}

fn emit_effects(
    mut commands: Commands,
    net: Res<NetGame>,
    level: Option<ResMut<Level>>,
    types: Option<Res<VehicleTypes>>,
    effect_assets: Res<EffectAssets>,
    mut particles: ResMut<Particles>,
    mut happenings: MessageReader<NetEvent>,
    pieces: Query<Entity, With<Piece>>,
    mut panes: Query<(&BlockPanes, &mut Mesh3d, &mut Visibility)>,
    mut meshes: ResMut<Assets<Mesh>>,
    asphalt: Option<Res<AsphaltMaterial>>,
) {
    if particles.round != net.round {
        *particles = Particles { round: net.round, ..default() };
        pieces.iter().for_each(|e| commands.entity(e).despawn());
    }

    let fresh: Vec<NetEvent> = happenings.read().cloned().collect();
    let Some(mut level) = level else { return };
    let small = || (rand::random::<u32>() & 0x1f) as f32 * 0.000_244_140_63 + 0.015625;
    let blood = || (rand::random::<u32>() & 0x3f) as f32 * 0.000_976_562_5 + 0.0625;
    let mut spawn_piece = |particles: &mut Particles, kind: PieceKind, pos: Vec3, vel: Vec3, axes: [Vec3; 3]| {
        if particles.pieces >= MAX_PIECES {
            return;
        }

        let turn = (rand::random_range(-1.0f32..1.0) * 4.0 + 12.0) / 60.0;
        let back = -(rand::random_range(-1.0f32..1.0) * 4.0 + 12.0) / 60.0;
        let spin = axes[1] * turn + axes[0] * back;
        let (mesh, material) = if kind == PieceKind::Case { effect_assets.case.clone() } else { (effect_assets.chip.0.clone(), asphalt.as_ref().map_or(effect_assets.chip.1.clone(), |a| a.0.clone())) };

        particles.pieces += 1;
        commands.spawn((
            Piece { kind, pos, vel, axes, spin, was: (pos, axes) },
            Mesh3d(mesh),
            MeshMaterial3d(material),
            piece_transform(pos, &axes),
            NotShadowCaster,
            DespawnOnExit(Screen::InGame),
        ));
    };

    for happening in fresh {
        match happening {
            NetEvent::Gunshot { item, pos, .. } => {
                let Some(it) = usize::try_from(item).ok().and_then(|i| net.items.get(i)) else { continue };
                let turn = it.rot.conjugate();
                let axes = [turn * Vec3::X, turn * Vec3::Y, turn * Vec3::Z];
                let vel = (axes[0] + axes[1]) * 0.025 + random_vec3(0.016_666_668, 0.0);

                spawn_piece(&mut particles, PieceKind::Case, pos, vel, axes);
            }
            NetEvent::BulletHit { hit: RICOCHET, pos, normal } => {
                for _ in 0..3 {
                    spawn_piece(&mut particles, PieceKind::Chip, pos, normal * 0.016_666_668 + random_vec3(0.05, 0.0), [Vec3::X, Vec3::Y, Vec3::Z]);
                }
            }
            NetEvent::BulletHit { hit: hit @ (BODY | GRAZE), pos, .. } => {
                for _ in 0..if hit == BODY { 16 } else { 4 } {
                    let size = blood();
                    let mut p = Particle::new(Kind::Blood, pos, random_vec3(0.033_333_335, 0.008_333_334));
                    p.size = size * 0.25;
                    particles.add(p);
                }
            }
            NetEvent::BulletHit { hit: METAL, pos, .. } => {
                for _ in 0..8 {
                    let mut p = Particle::new(Kind::Smoke, pos, random_vec3(0.033_333_335, 0.008_333_334));
                    (p.life, p.size, p.drag) = (SMOKE_LIFE, small(), SMOKE_DRAG);
                    particles.add(p);
                }
            }
            NetEvent::Explosion { pos } => {
                for _ in 0..0x80 {
                    let mut p = Particle::new(Kind::Smoke, pos, random_vec3(0.066_666_67, 0.333_333_34));
                    (p.life, p.size, p.drag) = (SMOKE_LIFE, small(), SMOKE_DRAG);
                    particles.add(p);
                }
            }
            NetEvent::WindowBroke { vehicle, window, pos, vel } => {
                let Some(v) = usize::try_from(vehicle).ok().and_then(|v| net.vehicles.get(v)) else { continue };
                let Some(tris) = types.as_ref().and_then(|t| t.0.get(v.kind as usize)).and_then(|t| usize::try_from(window).ok().and_then(|w| t.windows.get(w))) else { continue };
                let Some(first) = tris.first() else { continue };
                let last = tris.get(1).map_or(first[2], |t| t[2]);
                let turn = v.rot.conjugate();
                let corners = [first[0], first[1], first[2], last].map(|c| v.track.pos + turn * c);

                shatter_window(&mut particles, corners, pos, vel);
            }
            NetEvent::BulletHole { pos, vel, cell, face } if face >= 0 => {
                if !break_pane(&mut particles, &mut level, cell, face, pos, vel) {
                    continue;
                }

                let origin = IVec3::new(cell.x & !7, cell.y & !7, cell.z & !7);

                for (block, mut mesh, mut visibility) in &mut panes {
                    if block.origin != origin {
                        continue;
                    }

                    let part = blocks::panes(&level.map, origin);

                    if part.indices.is_empty() {
                        *visibility = Visibility::Hidden;
                    } else {
                        mesh.0 = meshes.add(centred_mesh(part, block.centre));
                    }
                }
            }
            _ => {}
        }
    }
}

fn settle(p: &mut Particle, map: &Map) {
    for k in 0..3 {
        p.corner_vel[k].y -= p.gravity;

        let old = p.frame[k];
        p.frame[k] += p.corner_vel[k];

        if let Some((at, n)) = trace(map, old, p.frame[k]) {
            p.frame[k] = at + n * RESTING_LIFT;
            p.corner_vel[k] = bounce(p.corner_vel[k], n, 0.875);
        }
    }

    let snap = p.corner_vel;

    for j in 0..3 {
        let next = (j + 1) % 3;
        let d = p.frame[next] - p.frame[j];
        let dir = d.normalize_or_zero();
        let push = dir.dot(snap[next]) * 0.25 + ((d.length() - p.rest[j]) * 0.125 - dir.dot(snap[j]) * 0.25);

        p.corner_vel[j] += dir * push;
        p.corner_vel[next] -= dir * push;
    }
}

fn simulate_effects(mut net: ResMut<NetGame>, level: Option<Res<Level>>, mut particles: ResMut<Particles>, mut pieces: Query<&mut Piece>) {
    if matches!(net.game_state, GameState::Restarting | GameState::Paused) {
        return;
    }

    let Some(level) = level else { return };
    let map = &level.map;

    for p in &mut particles.list {
        if p.kind == Kind::Smoke {
            p.vel = Vec3::new(p.vel.x, p.vel.y - p.gravity, p.vel.z) * p.drag;

            let old = p.pos;
            p.pos += p.vel;

            if let Some((at, n)) = trace(map, old, p.pos) {
                p.pos = at + n * LIFT;
                p.vel = bounce(p.vel, n, 0.9375);
            }

            continue;
        }

        if p.kind == Kind::Triangle {
            if p.life > SETTLING {
                settle(p, map);
            }

            continue;
        }

        if p.drag <= 0.0 {
            continue;
        }

        p.vel = Vec3::new(p.vel.x, p.vel.y - p.gravity, p.vel.z) * p.drag;

        let old = p.pos;
        p.pos += p.vel;

        if p.kind == Kind::Shard && p.spin > 0.004_166_667 {
            spin_axes(&mut p.frame, p.spin_axis, p.spin);
        }

        match trace(map, old, p.pos) {
            Some((at, n)) if p.kind != Kind::Shard || n.y > 0.0 => {
                p.pos = at + n * LIFT;
                (p.vel, p.drag, p.life) = (Vec3::ZERO, 0.0, 0);
            }
            Some((at, n)) => {
                p.pos = at + n * LIFT;
                p.vel = bounce(p.vel, n, 0.9375);
            }
            None => {}
        }
    }

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
                let pitch = (rand::random::<u32>() & 0x7f) as f32 * 0.000_976_562_5 + 1.0;
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

    particles.list.retain_mut(|p| {
        if p.life < FOREVER {
            p.life -= 1;
        }

        p.life > 0
    });
}

#[derive(Default)]
struct Batch {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colours: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl Batch {
    fn quad(&mut self, corners: [Vec3; 4], normal: Vec3, colour: [f32; 4]) {
        let base = self.positions.len() as u32;

        for (c, uv) in corners.into_iter().zip([[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]) {
            self.positions.push(c.to_array());
            self.normals.push(normal.to_array());
            self.uvs.push(uv);
            self.colours.push(colour);
        }

        self.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn triangle(&mut self, corners: [Vec3; 3]) {
        let base = self.positions.len() as u32;
        let normal = (corners[1] - corners[0]).cross(corners[2] - corners[0]).normalize_or_zero();

        for c in corners {
            self.positions.push(c.to_array());
            self.normals.push(normal.to_array());
            self.uvs.push([0.0, 0.0]);
            self.colours.push([1.0; 4]);
        }

        self.indices.extend([base, base + 1, base + 2]);
    }

    fn write(self, mesh: &mut Mesh) {
        if self.indices.is_empty() {
            *mesh = empty_mesh();
            return;
        }

        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colours);
        mesh.insert_indices(Indices::U32(self.indices));
    }
}

fn place_pieces(time: Res<Time<Fixed>>, mut pieces: Query<(&Piece, &mut Transform)>) {
    let a = time.overstep_fraction();

    for (piece, mut transform) in &mut pieces {
        let (from, to) = (piece_transform(piece.was.0, &piece.was.1), piece_transform(piece.pos, &piece.axes));
        let place = Transform::from_translation(from.translation.lerp(to.translation, a)).with_rotation(from.rotation.slerp(to.rotation, a));

        if *transform != place {
            *transform = place;
        }
    }
}

fn draw_particles(particles: Res<Particles>, effect_assets: Res<EffectAssets>, camera: Option<Single<&Transform, With<GameCamera>>>, mut meshes: ResMut<Assets<Mesh>>) {
    let Some(camera) = camera else { return };
    let (right, up, back) = (camera.right().as_vec3(), camera.up().as_vec3(), camera.back().as_vec3());
    let (mut spots, mut blood, mut glass) = (Batch::default(), Batch::default(), Batch::default());

    for p in &particles.list {
        let fade = (p.life as f32 * 0.03125).min(1.0);
        let corners = |side: Vec3, up: Vec3, s: f32| [p.pos - side * s + up * s, p.pos + side * s + up * s, p.pos + side * s - up * s, p.pos - side * s - up * s];

        match p.kind {
            Kind::Smoke => spots.quad(corners(right, up, p.size), back, [SMOKE_COLOUR[0], SMOKE_COLOUR[1], SMOKE_COLOUR[2], fade]),
            Kind::Blood => blood.quad(corners(right, up, p.size), back, [1.0, 1.0, 1.0, fade]),
            Kind::Shard => {
                let c = corners(p.frame[0], p.frame[1], p.size);
                glass.triangle([c[0], c[1], c[2]]);
                glass.triangle([c[0], c[2], c[3]]);
            }
            Kind::Triangle => glass.triangle(p.frame),
        }
    }

    if let Some(mut mesh) = meshes.get_mut(&effect_assets.spots) {
        spots.write(&mut mesh);
    }

    if let Some(mut mesh) = meshes.get_mut(&effect_assets.blood) {
        blood.write(&mut mesh);
    }

    if let Some(mut mesh) = meshes.get_mut(&effect_assets.glass) {
        glass.write(&mut mesh);
    }
}
