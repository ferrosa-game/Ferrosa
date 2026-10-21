mod draw;
mod glass;
mod pieces;

use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    gltf::GltfAssetLabel,
    image::ImageSampler,
    light::NotShadowCaster,
    mesh::PrimitiveTopology,
    prelude::*,
};
use ferrosa_protocol::clientbound::game::GameState;
use ferrosa_world::{map::Map, trace::line_intersect_level};

use self::pieces::{Piece, PieceKind};
use super::{
    Level, blocks,
    items::ItemTypes,
    level_render::{AsphaltMaterial, BlockPanes, centred_mesh},
    netgame::{NetEvent, NetGame},
    network::NetTick,
    smoothing::Smoothing,
    textures,
    vehicles::VehicleTypes,
};
use crate::menu::Screen;

const MAX_PARTICLES: usize = 0x4000;
const FOREVER: i32 = 0xffff;
const GRAVITY: f32 = 9.8 / 60.0 / 60.0;
const DRAG: f32 = 399.0 / 400.0;
const SMOKE_DRAG: f32 = 0.975;
const SMOKE_LIFE: i32 = 0x3c;
pub(super) const FLASH_LIFE: i32 = 4;
const LIFT: f32 = 1.0 / 64.0;
const RESTING_LIFT: f32 = 1.0 / 512.0;
const UPRIGHT: f32 = 0.707;
const RICOCHET: i32 = 0;
const BODY: i32 = 1;
const METAL: i32 = 2;
const GRAZE: i32 = 3;
const CASE_MODEL: &str = "models/items/case.glb";
const CHIP_MODEL: &str = "models/items/chip.glb";
const CASE_METALLIC: f32 = 0.5;
const CASE_ROUGHNESS: f32 = 0.125;
const CHIP_COLOUR: Color = Color::srgb(0.3, 0.3, 0.3);
const BLOOD_SHADE: Color = Color::srgb(0.25, 0.0, 0.0);
const GLASS: Color = Color::srgba(0.6, 0.7, 0.75, 0.35);

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Blood,
    Smoke,
    Shard,
    Triangle,
    Flash,
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
    gun: Option<usize>,
}

impl Particle {
    fn new(kind: Kind, pos: Vec3, vel: Vec3) -> Self {
        Self { kind, life: FOREVER, gravity: GRAVITY, drag: DRAG, size: 0.0, pos, vel, frame: [Vec3::ZERO; 3], corner_vel: [Vec3::ZERO; 3], rest: [0.0; 3], spin_axis: Vec3::ZERO, spin: 0.0, gun: None }
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

#[derive(Resource)]
struct EffectAssets {
    case: (Handle<Mesh>, Handle<StandardMaterial>),
    chip: (Handle<Mesh>, Handle<StandardMaterial>),
    spots: Handle<Mesh>,
    blood: Handle<Mesh>,
    glass: Handle<Mesh>,
    flash: Handle<Mesh>,
}

pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Particles>()
            .add_systems(OnEnter(Screen::InGame), spawn_effect_assets)
            .add_systems(
                FixedUpdate,
                (emit_effects, simulate_particles, pieces::simulate_pieces).chain().in_set(NetTick::Follow).run_if(in_state(Screen::InGame)).run_if(resource_exists::<EffectAssets>),
            )
            .add_systems(Update, (draw::draw_particles.after(Smoothing::Glide), draw::light_flashes.after(Smoothing::Glide), pieces::place_pieces).run_if(in_state(Screen::InGame)).run_if(resource_exists::<EffectAssets>))
            .add_systems(OnExit(Screen::InGame), |mut commands: Commands, mut particles: ResMut<Particles>| {
                *particles = Particles::default();
                commands.remove_resource::<EffectAssets>();
            });
    }
}

pub fn empty_mesh(topology: PrimitiveTopology, vertices: usize) -> Mesh {
    Mesh::new(topology, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; vertices])
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0f32]; vertices])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32; 2]; vertices])
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0f32; 4]; vertices])
}

fn spawn_effect_assets(mut commands: Commands, assets: Res<AssetServer>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut images: ResMut<Assets<Image>>) {
    let primitive = |path: &str| assets.load(GltfAssetLabel::Primitive { mesh: 0, primitive: 0 }.from_asset(path.to_string()));
    let case_texture = assets.load(GltfAssetLabel::Texture(0).from_asset(CASE_MODEL));
    let case_material = materials.add(StandardMaterial { base_color_texture: Some(case_texture), metallic: CASE_METALLIC, perceptual_roughness: CASE_ROUGHNESS, ..default() });
    let chip_material = materials.add(StandardMaterial { base_color: CHIP_COLOUR, perceptual_roughness: 0.9, ..default() });

    let mut spot = textures::particle().into_image();
    spot.sampler = ImageSampler::linear();

    let spots = meshes.add(empty_mesh(PrimitiveTopology::TriangleList, 3));
    let blood = meshes.add(empty_mesh(PrimitiveTopology::TriangleList, 3));
    let glass = meshes.add(empty_mesh(PrimitiveTopology::TriangleList, 3));
    let flash = meshes.add(empty_mesh(PrimitiveTopology::TriangleList, 3));
    let spot_image = images.add(spot);
    let spot_material = StandardMaterial { base_color: Color::WHITE, base_color_texture: Some(spot_image.clone()), unlit: true, alpha_mode: AlphaMode::Blend, cull_mode: None, ..default() };
    let flash_material = StandardMaterial { base_color: Color::WHITE, base_color_texture: Some(spot_image.clone()), unlit: true, alpha_mode: AlphaMode::Add, cull_mode: None, ..default() };
    let blood_material = StandardMaterial { base_color: BLOOD_SHADE, base_color_texture: Some(spot_image), perceptual_roughness: 1.0, alpha_mode: AlphaMode::Blend, cull_mode: None, ..default() };
    let glass_material = StandardMaterial { base_color: GLASS, alpha_mode: AlphaMode::Blend, perceptual_roughness: 0.05, reflectance: 1.0, double_sided: true, cull_mode: None, ..default() };

    commands.spawn((Mesh3d(spots.clone()), MeshMaterial3d(materials.add(spot_material)), NoFrustumCulling, NotShadowCaster, DespawnOnExit(Screen::InGame)));
    commands.spawn((Mesh3d(blood.clone()), MeshMaterial3d(materials.add(blood_material)), NoFrustumCulling, NotShadowCaster, DespawnOnExit(Screen::InGame)));
    commands.spawn((Mesh3d(glass.clone()), MeshMaterial3d(materials.add(glass_material)), NoFrustumCulling, NotShadowCaster, DespawnOnExit(Screen::InGame)));
    commands.spawn((Mesh3d(flash.clone()), MeshMaterial3d(materials.add(flash_material)), NoFrustumCulling, NotShadowCaster, DespawnOnExit(Screen::InGame)));

    for _ in 0..draw::FLASH_LIGHTS {
        let light = PointLight { color: draw::FLASH_LIGHT_COLOUR, intensity: 0.0, range: draw::FLASH_RANGE, shadow_maps_enabled: false, ..default() };
        commands.spawn((light, Transform::default(), Visibility::Hidden, draw::FlashLight, DespawnOnExit(Screen::InGame)));
    }

    commands.insert_resource(EffectAssets { case: (primitive(CASE_MODEL), case_material), chip: (primitive(CHIP_MODEL), chip_material), spots, blood, glass, flash });
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

fn spin_axes(axes: &mut [Vec3; 3], axis: Vec3, angle: f32) {
    let turn = Quat::from_axis_angle(axis, angle);

    axes.iter_mut().for_each(|a| *a = turn * *a);
}

fn emit_effects(
    mut commands: Commands,
    net: Res<NetGame>,
    level: Option<ResMut<Level>>,
    types: Option<Res<VehicleTypes>>,
    item_types: Option<Res<ItemTypes>>,
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
    let small = || (rand::random::<u32>() & 0x1f) as f32 / 4096.0 + 1.0 / 64.0;
    let blood = || (rand::random::<u32>() & 0x3f) as f32 / 1024.0 + 1.0 / 16.0;
    let asphalt = asphalt.as_deref();

    for happening in fresh {
        match happening {
            NetEvent::Gunshot { kind, item, pos, .. } => {
                let Some(gun) = usize::try_from(item).ok() else { continue };
                let Some(it) = net.items.get(gun) else { continue };
                let turn = it.rot.conjugate();
                let axes = [turn * Vec3::X, turn * Vec3::Y, turn * Vec3::Z];
                let vel = (axes[0] + axes[1]) * 0.025 + random_vec3(1.0 / 60.0, 0.0);

                pieces::spawn_piece(&mut commands, &effect_assets, asphalt, &mut particles, PieceKind::Case, pos, vel, axes);

                let muzzle = kind.and_then(|k| item_types.as_ref()?.0.get(k as usize)?.muzzle_flash);

                if let Some(muzzle) = muzzle {
                    let mut p = Particle::new(Kind::Flash, muzzle, Vec3::ZERO);
                    (p.life, p.drag, p.gun) = (FLASH_LIFE, 0.0, Some(gun));

                    particles.add(p);
                }
            }
            NetEvent::BulletHit { hit: RICOCHET, pos, normal } => {
                for _ in 0..3 {
                    let vel = normal * (1.0 / 60.0) + random_vec3(0.05, 0.0);

                    pieces::spawn_piece(&mut commands, &effect_assets, asphalt, &mut particles, PieceKind::Chip, pos, vel, [Vec3::X, Vec3::Y, Vec3::Z]);
                }
            }
            NetEvent::BulletHit { hit: hit @ (BODY | GRAZE), pos, .. } => {
                for _ in 0..if hit == BODY { 16 } else { 4 } {
                    let size = blood();
                    let mut p = Particle::new(Kind::Blood, pos, random_vec3(1.0 / 30.0, 1.0 / 120.0));
                    p.size = size * 0.25;
                    particles.add(p);
                }
            }
            NetEvent::BulletHit { hit: METAL, pos, .. } => {
                for _ in 0..8 {
                    let mut p = Particle::new(Kind::Smoke, pos, random_vec3(1.0 / 30.0, 1.0 / 120.0));
                    (p.life, p.size, p.drag) = (SMOKE_LIFE, small(), SMOKE_DRAG);
                    particles.add(p);
                }
            }
            NetEvent::Explosion { pos } => {
                for _ in 0..0x80 {
                    let mut p = Particle::new(Kind::Smoke, pos, random_vec3(1.0 / 15.0, 1.0 / 3.0));
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

                glass::shatter_window(&mut particles, corners, pos, vel);
            }
            NetEvent::BulletHole { pos, vel, cell, face } if face >= 0 => {
                if !glass::break_pane(&mut particles, &mut level, cell, face, pos, vel) {
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

fn simulate_particles(net: Res<NetGame>, level: Option<Res<Level>>, mut particles: ResMut<Particles>) {
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
                p.vel = bounce(p.vel, n, 15.0 / 16.0);
            }

            continue;
        }

        if p.kind == Kind::Triangle {
            glass::settle(p, map);

            continue;
        }

        if p.drag <= 0.0 {
            continue;
        }

        p.vel = Vec3::new(p.vel.x, p.vel.y - p.gravity, p.vel.z) * p.drag;

        let old = p.pos;
        p.pos += p.vel;

        if p.kind == Kind::Shard && p.spin > 1.0 / 240.0 {
            spin_axes(&mut p.frame, p.spin_axis, p.spin);
        }

        match trace(map, old, p.pos) {
            Some((at, n)) if p.kind != Kind::Shard || n.y > 0.0 => {
                p.pos = at + n * LIFT;
                (p.vel, p.drag, p.life) = (Vec3::ZERO, 0.0, 0);
            }
            Some((at, n)) => {
                p.pos = at + n * LIFT;
                p.vel = bounce(p.vel, n, 15.0 / 16.0);
            }
            None => {}
        }
    }

    particles.list.retain_mut(|p| {
        if p.life < FOREVER {
            p.life -= 1;
        }

        p.life > 0
    });
}
