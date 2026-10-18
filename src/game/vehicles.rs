use std::f32::consts::{FRAC_PI_2, PI};

use bevy::{
    gltf::{GltfAssetLabel, GltfMaterialName},
    math::Affine2,
    platform::collections::HashMap,
    prelude::*,
};
use ferrosa_protocol::clientbound::game::VehicleKind;
use ferrosa_world::{
    map::Map,
    trace::line_intersect_level,
    vehicle_body::PHYSICS_PREFIXES,
    vehicle_types::{VehicleType, model_name},
};

use super::{
    GAME_FPS, Level,
    netgame::{NetGame, entities::Track, entities::Vehicle},
    smoothing::{Blend, Smoothing, Snapshots, glide},
};
use crate::menu::Screen;

const GLASS: Color = Color::srgba(0.08, 0.1, 0.12, 0.55);
const CLEAR_GLASS: Color = Color::srgba(1.0, 1.0, 1.0, 0.04);
const COLUMNS: u32 = 16;
const BURNT: u32 = 6;
const ROTOR_HEIGHT: f32 = 1.5;
const TRAFFIC_BACK: f32 = 1.5;
const TRAFFIC_AHEAD: f32 = 3.0;
const WHEEL_SINK: f32 = 0.875;
const CARRY_ON: f32 = 64.0;

#[derive(Resource)]
pub struct VehicleTypes(pub Vec<VehicleType>);

impl VehicleTypes {
    pub fn get(&self, kind: VehicleKind) -> Option<&VehicleType> {
        self.0.get(kind as usize)
    }

    fn wheels(&self, kind: VehicleKind) -> &[ferrosa_world::vehicle_types::WheelDef] {
        self.get(kind).map_or(&[], |t| &t.wheels)
    }
}

#[derive(Component)]
pub struct NetVehicle {
    pub index: usize,
    kind: VehicleKind,
}

#[derive(Component, Clone, Copy, PartialEq)]
struct WheelPose {
    steer: f32,
    heights: [f32; 4],
}

impl WheelPose {
    fn of(v: &Vehicle) -> Self {
        Self { steer: v.steer, heights: v.wheels.map(|w| w[0]) }
    }
}

impl Blend for WheelPose {
    fn blend(&self, to: &Self, t: f32) -> Self {
        Self { steer: self.steer.lerp(to.steer, t), heights: std::array::from_fn(|k| self.heights[k].lerp(to.heights[k], t)) }
    }
}

#[derive(Component)]
struct NetTraffic {
    index: usize,
    kind: VehicleKind,
}

#[derive(Component)]
struct Wheel {
    index: usize,
    model: usize,
    angle: f32,
}

#[derive(Component)]
struct Rotor(f32);

#[derive(Component, Clone, Copy, PartialEq, Eq)]
struct Paint(u32);

#[derive(Component)]
struct PaintSlot(Handle<StandardMaterial>);

#[derive(Component)]
struct CarWindow(usize);

#[derive(Resource)]
struct Glass {
    tinted: Handle<StandardMaterial>,
    clear: Handle<StandardMaterial>,
}

#[derive(Component)]
struct Pane;

impl FromWorld for Glass {
    fn from_world(world: &mut World) -> Self {
        let glass = |base_color| StandardMaterial { base_color, alpha_mode: AlphaMode::Blend, perceptual_roughness: 0.05, reflectance: 0.5, cull_mode: None, ..default() };
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();

        Self { tinted: materials.add(glass(GLASS)), clear: materials.add(glass(CLEAR_GLASS)) }
    }
}

#[derive(Resource, Default)]
struct PaintMaterials(HashMap<(AssetId<StandardMaterial>, u32), Handle<StandardMaterial>>);

pub struct VehiclesPlugin;

impl Plugin for VehiclesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PaintMaterials>()
            .init_resource::<Glass>()
            .add_systems(Update, (sync_vehicles, sync_traffic).in_set(Smoothing::Sync).run_if(resource_exists::<NetGame>))
            .add_systems(Update, glide::<WheelPose>.in_set(Smoothing::Glide))
            .add_systems(Update, turn_wheels.after(Smoothing::Glide).run_if(in_state(Screen::InGame)).run_if(resource_exists::<NetGame>))
            .add_systems(Update, (hide_physics, glaze, mark_paint, paint).chain().after(Smoothing::Glide).run_if(in_state(Screen::InGame)))
            .add_systems(Update, clear_own_glass.after(glaze).run_if(in_state(Screen::InGame)).run_if(resource_exists::<NetGame>))
            .add_systems(Update, break_windows.after(hide_physics).run_if(in_state(Screen::InGame)).run_if(resource_exists::<NetGame>));
    }
}

fn scene(assets: &AssetServer, path: String) -> WorldAssetRoot {
    WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(path)))
}

fn vehicle_model(kind: VehicleKind, far: bool) -> String {
    let name = model_name(kind);
    let far_model = format!("models/vehicles/{name}_far.glb");

    if far && std::path::Path::new("data").join(&far_model).exists() { far_model } else { format!("models/vehicles/{name}.glb") }
}

fn wheel_model(model: usize) -> String {
    format!("models/vehicles/wheel_{model}.glb")
}

fn rotation(q: Quat) -> Quat {
    q.conjugate()
}

fn paint_of(health: i32, color: u32) -> Paint {
    Paint(if health <= 0 { BURNT } else if color < COLUMNS { color } else { 0 })
}

fn spawn_vehicle(commands: &mut Commands, assets: &AssetServer, types: &VehicleTypes, kind: VehicleKind, root: impl Bundle) {
    let body = vehicle_model(kind, false);
    let vehicle = commands.spawn((root, Visibility::default(), DespawnOnExit(Screen::InGame))).id();

    commands.spawn((scene(assets, body), ChildOf(vehicle)));

    for (index, w) in types.wheels(kind).iter().enumerate() {
        let model = w.drive as usize;
        let wheel = commands.spawn((Wheel { index, model, angle: 0.0 }, Transform::from_translation(w.mount), Visibility::default(), ChildOf(vehicle))).id();

        commands.spawn((scene(assets, wheel_model(model)), ChildOf(wheel)));
    }

    if kind == VehicleKind::Helicopter {
        for half in [0.0, PI] {
            commands.spawn((Rotor(half), scene(assets, "models/vehicles/rotor.glb".into()), Transform::from_xyz(0.0, ROTOR_HEIGHT, 0.0), ChildOf(vehicle)));
        }
    }
}

fn sync_vehicles(
    mut commands: Commands,
    assets: Res<AssetServer>,
    net: Res<NetGame>,
    types: Res<VehicleTypes>,
    mut vehicles: Query<(Entity, &NetVehicle, &mut Paint, &mut Snapshots<Transform>, &mut Snapshots<WheelPose>)>,
) {
    let mut drawn = vec![false; net.vehicles.len()];

    for (e, nv, mut paint, mut places, mut wheel_poses) in &mut vehicles {
        let Some(v) = net.vehicles.get(nv.index).filter(|v| v.active && v.kind == nv.kind) else {
            commands.entity(e).despawn();
            continue;
        };

        drawn[nv.index] = true;
        paint.set_if_neq(paint_of(v.health, v.color));

        if places.last_tick() != Some(v.track.tick) {
            places.push(v.track.tick, Transform::from_translation(v.track.pos).with_rotation(rotation(v.rot)));
            wheel_poses.push(v.track.tick, WheelPose::of(v));
        }
    }

    for (i, v) in net.vehicles.iter().enumerate().filter(|(i, v)| v.active && !drawn[*i]) {
        let (place, wheel_pose) = (Transform::from_translation(v.track.pos).with_rotation(rotation(v.rot)), WheelPose::of(v));
        let snapshots = (Snapshots::new(v.track.tick, place), Snapshots::new(v.track.tick, wheel_pose));
        let root = (NetVehicle { index: i, kind: v.kind }, paint_of(v.health, v.color), place, wheel_pose, snapshots);

        spawn_vehicle(&mut commands, &assets, &types, v.kind, root);
    }
}

fn turn_wheels(
    mut commands: Commands,
    assets: Res<AssetServer>,
    time: Res<Time>,
    net: Res<NetGame>,
    types: Res<VehicleTypes>,
    vehicles: Query<(&NetVehicle, &WheelPose, &Children)>,
    mut wheels: Query<(Entity, &mut Wheel, &mut Transform, &Children), Without<Rotor>>,
    mut rotors: Query<(&Rotor, &mut Transform), Without<Wheel>>,
) {
    let frames = time.delta_secs() * GAME_FPS;

    for (nv, pose, children) in &vehicles {
        let Some(v) = net.vehicles.get(nv.index) else { continue };

        for &child in children {
            if let Ok((wheel_entity, mut wheel, mut wheel_transform, wheel_children)) = wheels.get_mut(child) {
                let Some(mount) = types.wheels(v.kind).get(wheel.index) else { continue };
                let spin = v.wheels.get(wheel.index).map_or(0.0, |w| w[1]);
                let model = mount.drive as usize | (v.burst >> wheel.index & 1) as usize;

                if model != wheel.model {
                    wheel.model = model;
                    wheel_children.iter().for_each(|c| commands.entity(c).despawn());
                    commands.spawn((scene(&assets, wheel_model(model)), ChildOf(wheel_entity)));
                }

                wheel.angle += spin * frames;

                let steer = if wheel.index < 2 { pose.steer } else { 0.0 };
                let height = pose.heights.get(wheel.index).copied().unwrap_or_default();
                *wheel_transform = Transform::from_translation(mount.mount + Vec3::Y * height).with_rotation(Quat::from_rotation_y(-steer) * Quat::from_rotation_x(-wheel.angle));
            } else if let Ok((&Rotor(half), mut rotor_transform)) = rotors.get_mut(child) {
                rotor_transform.rotation = Quat::from_rotation_y(-(v.wheels[0][0] + half));
            }
        }
    }
}

fn place_traffic(map: &Map, ty: Option<&VehicleType>, pos: Vec3, yaw: f32) -> Transform {
    let ground = |from: Vec3, down: f32| line_intersect_level(&map.ground, &map.level.area, &map.level.meshes, from, from - Vec3::Y * down).map(|h| h.hit.pos.y);
    let lift = ty.and_then(|t| t.wheels.first()).map_or(0.0, |w| WHEEL_SINK * w.radius - w.mount.y);

    let at = Vec3::new(pos.x, ground(pos + Vec3::Y * 2.0, 10.0).map_or(pos.y, |y| y + lift), pos.z);
    let mut rotation = Quat::from_rotation_y(-(FRAC_PI_2 + yaw));

    if let Some(ahead) = ground(at - rotation * Vec3::Z * TRAFFIC_AHEAD, 32.0) {
        rotation = Quat::from_axis_angle(rotation * Vec3::X, (ahead + lift - at.y).atan2(TRAFFIC_AHEAD)) * rotation;
    }

    Transform::from_translation(at - rotation * Vec3::Z * TRAFFIC_BACK).with_rotation(rotation)
}

fn carried_on(track: &Track, now: i32) -> Vec3 {
    let gap = track.tick - track.prev_tick;
    let moved = track.pos - track.prev_pos;
    let per_tick = if gap > 0 { moved / gap as f32 } else { moved };

    track.pos + per_tick * ((now - track.tick).max(0) as f32).min(CARRY_ON)
}

fn sync_traffic(
    mut commands: Commands,
    assets: Res<AssetServer>,
    net: Res<NetGame>,
    types: Res<VehicleTypes>,
    level: Res<Level>,
    mut cars: Query<(Entity, &NetTraffic, &mut Paint, &mut Snapshots<Transform>, &mut Visibility)>,
) {
    let count = (net.traffic_count as usize).min(net.traffic.len());
    let standing_in = |i: usize, vehicle: i32| usize::try_from(vehicle).ok().and_then(|v| net.vehicles.get(v)).is_some_and(|v| v.active && v.traffic_car == i as i32);
    let mut drawn = vec![false; count];

    for (e, nt, mut paint, mut places, mut visibility) in &mut cars {
        let Some(car) = net.traffic.get(nt.index).filter(|c| nt.index < count && c.kind == nt.kind) else {
            commands.entity(e).despawn();
            continue;
        };

        drawn[nt.index] = true;
        paint.set_if_neq(Paint(car.color.min(COLUMNS - 1)));

        if places.last_tick() != Some(net.tick) {
            places.push(net.tick, place_traffic(&level.map, types.get(car.kind), carried_on(&car.track, net.tick), car.yaw));
        }

        visibility.set_if_neq(if standing_in(nt.index, car.vehicle) { Visibility::Hidden } else { Visibility::Inherited });
    }

    for (i, car) in net.traffic.iter().enumerate().take(count).filter(|(i, _)| !drawn[*i]) {
        let body = vehicle_model(car.kind, true);
        let place = place_traffic(&level.map, types.get(car.kind), carried_on(&car.track, net.tick), car.yaw);
        let traffic = (NetTraffic { index: i, kind: car.kind }, Paint(car.color.min(COLUMNS - 1)), Snapshots::new(net.tick, place), place, Visibility::default());
        let e = commands.spawn((traffic, DespawnOnExit(Screen::InGame))).id();

        commands.spawn((scene(&assets, body), ChildOf(e)));

        for w in types.wheels(car.kind) {
            commands.spawn((scene(&assets, wheel_model(w.drive as usize)), Transform::from_translation(w.mount), ChildOf(e)));
        }
    }
}

fn window_of(name: &str) -> Option<usize> {
    name.strip_prefix("window.")?.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
}

fn hide_physics(mut commands: Commands, named: Query<(Entity, &Name), Added<Name>>) {
    for (e, name) in &named {
        if let Some(window) = window_of(name.as_str()) {
            commands.entity(e).insert(CarWindow(window));
        } else if PHYSICS_PREFIXES.iter().any(|p| name.as_str().starts_with(p)) {
            commands.entity(e).insert(Visibility::Hidden);
        }
    }
}

fn glaze(
    mut commands: Commands,
    glass: Res<Glass>,
    mut panes: Query<(Entity, &mut MeshMaterial3d<StandardMaterial>), Added<MeshMaterial3d<StandardMaterial>>>,
    names: Query<&Name>,
    parents: Query<&ChildOf>,
) {
    for (e, mut material) in &mut panes {
        let in_window = std::iter::once(e).chain(parents.iter_ancestors(e)).any(|a| names.get(a).is_ok_and(|n| window_of(n.as_str()).is_some()));

        if in_window {
            material.0 = glass.tinted.clone();
            commands.entity(e).insert(Pane);
        }
    }
}

fn clear_own_glass(
    net: Res<NetGame>,
    glass: Res<Glass>,
    mut panes: Query<(Entity, &mut MeshMaterial3d<StandardMaterial>), With<Pane>>,
    vehicles: Query<&NetVehicle>,
    parents: Query<&ChildOf>,
) {
    let own = net.me().and_then(|p| usize::try_from(p.human).ok()).and_then(|h| net.humans.get(h)).and_then(|h| usize::try_from(h.vehicle).ok());

    for (e, mut material) in &mut panes {
        let vehicle = parents.iter_ancestors(e).find_map(|a| vehicles.get(a).ok()).map(|nv| nv.index);
        let wanted = if vehicle.is_some() && vehicle == own { &glass.clear } else { &glass.tinted };

        if material.0 != *wanted {
            material.0 = wanted.clone();
        }
    }
}

fn break_windows(net: Res<NetGame>, mut windows: Query<(Entity, &CarWindow, &mut Visibility)>, vehicles: Query<&NetVehicle>, parents: Query<&ChildOf>) {
    for (e, &CarWindow(window), mut visibility) in &mut windows {
        let vehicle = parents.iter_ancestors(e).find_map(|a| vehicles.get(a).ok()).and_then(|nv| net.vehicles.get(nv.index));
        let broken = vehicle.is_some_and(|v| v.windows_broken.get(window).copied().unwrap_or(false));

        visibility.set_if_neq(if broken { Visibility::Hidden } else { Visibility::Inherited });
    }
}

fn mark_paint(mut commands: Commands, meshes: Query<(Entity, &GltfMaterialName, &MeshMaterial3d<StandardMaterial>), Added<GltfMaterialName>>) {
    for (e, name, material) in &meshes {
        if name.0 == "paint" {
            commands.entity(e).insert(PaintSlot(material.0.clone()));
        }
    }
}

fn paint(
    owners: Query<Ref<Paint>>,
    mut meshes: Query<(Entity, Ref<PaintSlot>, &mut MeshMaterial3d<StandardMaterial>)>,
    parents: Query<&ChildOf>,
    mut cache: ResMut<PaintMaterials>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, slot, mut material) in &mut meshes {
        let Some(paint) = parents.iter_ancestors(e).find_map(|a| owners.get(a).ok()) else { continue };

        if !slot.is_added() && !paint.is_changed() {
            continue;
        }

        let key = (slot.0.id(), paint.0);
        let painted = match cache.0.get(&key) {
            Some(h) => h.clone(),
            None => {
                let Some(mut m) = materials.get(&slot.0).cloned() else { continue };
                m.uv_transform = Affine2::from_translation(Vec2::new(paint.0 as f32 / COLUMNS as f32, 0.0));
                let h = materials.add(m);
                cache.0.insert(key, h.clone());
                h
            }
        };

        if material.0 != painted {
            material.0 = painted;
        }
    }
}
