use std::f32::consts::FRAC_PI_2;

use bevy::{
    gltf::{GltfAssetLabel, GltfMaterialName},
    light::NotShadowCaster,
    math::Affine2,
    platform::collections::HashMap,
    prelude::*,
};
use ferrosa_protocol::clientbound::game::{ItemKind, ItemTail};
use ferrosa_world::{
    item_types::{ItemType, model_name},
    skeleton::BoneId,
};

use super::{
    controls::flag,
    human_sync::{NetHuman, bone_at},
    netgame::{entities::Item, NetGame},
    smoothing::{Smoothing, Snapshots},
};
use crate::{character_model::HumanPose, menu::Screen};

const NOTES: [&str; 7] = ["one", "five", "ten", "twenty", "fifty", "hundred", "thousand"];

#[derive(Resource)]
pub struct ItemTypes(pub Vec<ItemType>);

#[derive(Component)]
pub struct NetItem {
    pub index: usize,
    pub kind: ItemKind,
}

#[derive(Component)]
struct OnBack(usize);

#[derive(Component, Clone, Copy, PartialEq)]
struct PhoneFace(bool);

#[derive(Component, Clone, Copy, PartialEq)]
struct CashStack {
    bills: i32,
    picked: i32,
    notes: u32,
    hand: Option<u32>,
}

#[derive(Component)]
struct ItemModel(Handle<WorldAsset>);

#[derive(Component)]
struct Bill(usize);

#[derive(Component)]
struct FaceSlot(Handle<StandardMaterial>);

#[derive(Component)]
struct Highlight(Entity);

#[derive(Resource)]
struct BillTextures(Vec<Handle<Image>>);

#[derive(Resource)]
struct HighlightMaterial(Handle<StandardMaterial>);

#[derive(Resource, Default)]
struct ItemMaterials(HashMap<(AssetId<StandardMaterial>, usize), Handle<StandardMaterial>>);

pub struct ItemsPlugin;

impl Plugin for ItemsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ItemMaterials>()
            .add_systems(Startup, |mut commands: Commands, assets: Res<AssetServer>, mut materials: ResMut<Assets<StandardMaterial>>| {
                commands.insert_resource(BillTextures(NOTES.iter().map(|n| assets.load(format!("textures/cash_{n}.png"))).collect()));
                commands.insert_resource(HighlightMaterial(materials.add(StandardMaterial { base_color: Color::srgba(1.0, 1.0, 1.0, 0.5), unlit: true, alpha_mode: AlphaMode::Blend, ..default() })));
            })
            .add_systems(Update, sync_items.in_set(Smoothing::Sync).run_if(resource_exists::<NetGame>))
            .add_systems(Update, (stack_cash, mark_faces, show_faces, print_bills).chain().after(Smoothing::Sync).run_if(in_state(Screen::InGame)))
            .add_systems(Update, carry_on_back.after(Smoothing::Glide).before(Smoothing::View).run_if(in_state(Screen::InGame)))
            .add_systems(
                Update,
                (highlight_target, paint_highlight).chain().after(Smoothing::Glide).before(Smoothing::View).run_if(in_state(Screen::InGame)).run_if(resource_exists::<NetGame>),
            );
    }
}

fn place(item: &Item) -> Transform {
    Transform::from_translation(item.track.pos).with_rotation(item.rot.conjugate())
}

fn stowed(item: &Item) -> bool {
    item.parent_human >= 0 && item.parent_slot > 2
}

fn out_in_world(net: &NetGame, item: &Item) -> bool {
    let parent = usize::try_from(item.parent_item).ok().and_then(|p| net.items.get(p));

    !stowed(item) && parent.is_none_or(|p| !stowed(p))
}

fn on_back(net: &NetGame, item: &Item) -> Option<usize> {
    let human = usize::try_from(item.parent_human).ok().filter(|_| item.parent_slot >= 2)?;

    net.humans.get(human).is_some_and(|h| h.active).then_some(human)
}

fn cash_stack(item: &Item) -> CashStack {
    let (bills, picked, notes) = match item.tail {
        ItemTail::Cash { bills, spread, codes } => (bills, spread, codes),
        _ => (0, 0, 0),
    };

    CashStack { bills, picked, notes, hand: (item.parent_human >= 0).then_some(item.parent_slot) }
}

fn item_model(assets: &AssetServer, types: &ItemTypes, kind: ItemKind) -> Option<Handle<WorldAsset>> {
    let ty = types.0.get(kind as usize)?;

    Some(assets.load(GltfAssetLabel::Scene(0).from_asset(format!("models/items/{}.glb", model_name(kind as usize, &ty.name)))))
}

fn sync_items(
    mut commands: Commands,
    assets: Res<AssetServer>,
    net: Res<NetGame>,
    types: Res<ItemTypes>,
    mut items: Query<(Entity, &NetItem, &mut Snapshots<Transform>, Option<&OnBack>, Option<&mut CashStack>, Option<&mut PhoneFace>)>,
) {
    let mut drawn = vec![false; net.items.len()];

    for (e, ni, mut places, back, cash, face) in &mut items {
        let Some(item) = net.items.get(ni.index).filter(|i| i.active && i.kind == Some(ni.kind) && out_in_world(&net, i)) else {
            commands.entity(e).despawn();
            continue;
        };

        drawn[ni.index] = true;

        if let Some(mut cash) = cash {
            cash.set_if_neq(cash_stack(item));
        }

        if let Some(mut face) = face {
            face.set_if_neq(PhoneFace(item.phone.2 == 1));
        }

        if places.last_tick() != Some(item.track.tick) {
            places.push(item.track.tick, place(item));
        }

        match (on_back(&net, item), back) {
            (Some(human), Some(&OnBack(was))) if human == was => {}
            (Some(human), _) => {
                commands.entity(e).insert(OnBack(human));
            }
            (None, Some(_)) => {
                commands.entity(e).remove::<OnBack>();
            }
            (None, None) => {}
        }
    }

    for (i, item) in net.items.iter().enumerate().filter(|(i, item)| item.active && !drawn[*i] && out_in_world(&net, item)) {
        let Some(kind) = item.kind else { continue };
        let Some(model) = item_model(&assets, &types, kind) else { continue };
        let mut entity = commands.spawn((NetItem { index: i, kind }, place(item), Snapshots::new(item.track.tick, place(item)), Visibility::default(), DespawnOnExit(Screen::InGame)));

        match kind {
            ItemKind::CashWorld => entity.insert((cash_stack(item), ItemModel(model))),
            ItemKind::Phone => entity.insert((PhoneFace(item.phone.2 == 1), WorldAssetRoot(model))),
            _ => entity.insert(WorldAssetRoot(model)),
        };

        if let Some(human) = on_back(&net, item) {
            entity.insert(OnBack(human));
        }
    }
}

fn stack_cash(mut commands: Commands, stacks: Query<(Entity, Ref<CashStack>, &ItemModel, Option<&Children>)>) {
    for (e, stack, model, children) in &stacks {
        if !stack.is_changed() {
            continue;
        }

        children.into_iter().flatten().for_each(|&c| commands.entity(c).despawn());

        let fan = match stack.hand {
            Some(0) => -(1.0 / 32.0),
            Some(1) => 1.0 / 32.0,
            _ => 0.0,
        };

        let turn = Quat::from_rotation_y(fan * FRAC_PI_2);
        let mut at = Transform::from_translation(Vec3::Y * stack.bills as f32 * (7.0 / 512.0) * 0.5);

        for k in 0..=stack.bills.max(0) {
            let note = (stack.notes >> (k * 3) & 7) as usize;
            commands.spawn((Bill(note), WorldAssetRoot(model.0.clone()), at, ChildOf(e)));

            at.translation -= at.rotation * Vec3::Y * (7.0 / 512.0);

            if stack.hand.is_some() {
                at.rotation *= turn;

                if stack.picked - 1 == k {
                    at.rotation *= turn;
                    at.translation += at.rotation * Vec3::X * fan;
                }
            }
        }
    }
}

fn mark_faces(
    mut commands: Commands,
    meshes: Query<(Entity, &GltfMaterialName, &MeshMaterial3d<StandardMaterial>), Added<GltfMaterialName>>,
    phones: Query<(), With<PhoneFace>>,
    parents: Query<&ChildOf>,
) {
    for (e, name, material) in &meshes {
        if name.0 == "skin" && parents.iter_ancestors(e).any(|a| phones.contains(a)) {
            commands.entity(e).try_insert(FaceSlot(material.0.clone()));
        }
    }
}

fn changed(
    cache: &mut ItemMaterials,
    materials: &mut Assets<StandardMaterial>,
    base: &Handle<StandardMaterial>,
    key: usize,
    change: impl FnOnce(&mut StandardMaterial),
) -> Option<Handle<StandardMaterial>> {
    if let Some(h) = cache.0.get(&(base.id(), key)) {
        return Some(h.clone());
    }

    let mut m = materials.get(base)?.clone();
    change(&mut m);
    let h = materials.add(m);
    cache.0.insert((base.id(), key), h.clone());

    Some(h)
}

fn show_faces(
    faces: Query<Ref<PhoneFace>>,
    mut meshes: Query<(Entity, Ref<FaceSlot>, &mut MeshMaterial3d<StandardMaterial>)>,
    parents: Query<&ChildOf>,
    mut cache: ResMut<ItemMaterials>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, slot, mut material) in &mut meshes {
        let Some(face) = parents.iter_ancestors(e).find_map(|a| faces.get(a).ok()) else { continue };

        if !slot.is_added() && !face.is_changed() {
            continue;
        }

        let shift = |m: &mut StandardMaterial| m.uv_transform = Affine2::from_translation(Vec2::new(3.0 / 16.0, 0.0));
        let wanted = if face.0 { changed(&mut cache, &mut materials, &slot.0, 0, shift) } else { Some(slot.0.clone()) };

        if let Some(wanted) = wanted.filter(|w| *w != material.0) {
            material.0 = wanted;
        }
    }
}

fn print_bills(
    mut meshes: Query<(Entity, &mut MeshMaterial3d<StandardMaterial>), Added<MeshMaterial3d<StandardMaterial>>>,
    bills: Query<&Bill>,
    parents: Query<&ChildOf>,
    textures: Res<BillTextures>,
    mut cache: ResMut<ItemMaterials>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, mut material) in &mut meshes {
        let Some(&Bill(note)) = parents.iter_ancestors(e).find_map(|a| bills.get(a).ok()) else { continue };
        let Some(texture) = textures.0.get(note) else { continue };

        if let Some(printed) = changed(&mut cache, &mut materials, &material.0, 1 + note, |m| m.base_color_texture = Some(texture.clone())) {
            material.0 = printed;
        }
    }
}

fn carry_on_back(mut items: Query<(&OnBack, &mut Transform), Without<NetHuman>>, humans: Query<(&NetHuman, &Transform, &HumanPose)>) {
    for (&OnBack(human), mut transform) in &mut items {
        let Some((_, at, pose)) = humans.iter().find(|(nh, ..)| nh.0 == human) else { continue };
        let (torso, turn) = bone_at(at.translation, pose, BoneId::Torso.index());
        let [x, y, z] = [Vec3::X, Vec3::Y, Vec3::Z].map(|axis| turn * axis);

        *transform = Transform::from_translation(torso + z * 0.25).with_rotation(Quat::from_mat3(&Mat3::from_cols(x, -z, y)));
    }
}

fn free(net: &NetGame, item: &Item) -> bool {
    item.active && item.parent_human < 0 && usize::try_from(item.parent_item).ok().and_then(|p| net.items.get(p)).is_none_or(|p| p.parent_human < 0)
}

fn segment_distance(a: Vec3, b: Vec3, p: Vec3) -> f32 {
    let ab = b - a;
    let t = if ab.length_squared() > 0.0 { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };

    (a + ab * t - p).length()
}

fn reachable_item(net: &NetGame, types: &ItemTypes, (eyes, turn): (Vec3, Quat)) -> Option<usize> {
    let p = net.me()?;
    let h = net.humans.get(usize::try_from(p.human).ok()?)?;
    let hand = (p.input_flags & flag::SHIFT != 0) as usize;

    if h.vehicle != -1 || (!h.inventory[hand].is_empty() && !h.inventory[hand ^ 1].is_empty()) {
        return None;
    }

    let end = eyes - turn * Vec3::Z * 1.5;
    let (target, _) = net
        .items
        .iter()
        .enumerate()
        .filter(|(_, it)| free(net, it))
        .map(|(i, it)| (i, segment_distance(eyes, end, it.track.pos)))
        .filter(|&(_, d)| d < 0.5)
        .min_by(|a, b| a.1.total_cmp(&b.1))?;
    let kind = net.items[target].kind.filter(|&k| k != ItemKind::TableTest)?;
    let held = h.inventory[hand].first().and_then(|&i| net.items.get(usize::try_from(i).ok()?)).and_then(|i| i.kind);

    match held {
        Some(k) => types.0.get(k as usize).is_some_and(|t| t.can_mount_to[kind as usize] != 0).then_some(target),
        None => Some(target),
    }
}

fn highlight_target(
    mut commands: Commands,
    assets: Res<AssetServer>,
    net: Res<NetGame>,
    types: Res<ItemTypes>,
    items: Query<(Entity, &NetItem, &Transform), Without<Highlight>>,
    humans: Query<(&NetHuman, &Transform, &HumanPose), Without<Highlight>>,
    mut highlights: Query<(Entity, &Highlight, &mut Transform)>,
) {
    let target = net
        .me()
        .and_then(|p| usize::try_from(p.human).ok())
        .and_then(|h| humans.iter().find(|(nh, ..)| nh.0 == h))
        .and_then(|(_, at, pose)| reachable_item(&net, &types, bone_at(at.translation, pose, BoneId::Head.index())))
        .and_then(|i| items.iter().find(|(_, ni, _)| ni.index == i));

    for (e, highlight, mut at) in &mut highlights {
        match target.filter(|(item, ..)| *item == highlight.0) {
            Some((_, _, place)) => *at = place.with_scale(Vec3::splat(1.125)),
            None => commands.entity(e).despawn(),
        }
    }

    let Some((item, ni, place)) = target.filter(|(item, ..)| !highlights.iter().any(|(_, h, _)| h.0 == *item)) else { return };
    let Some(model) = item_model(&assets, &types, ni.kind) else { return };

    commands.spawn((Highlight(item), WorldAssetRoot(model), place.with_scale(Vec3::splat(1.125)), DespawnOnExit(Screen::InGame)));
}

fn paint_highlight(
    mut commands: Commands,
    mut meshes: Query<(Entity, &mut MeshMaterial3d<StandardMaterial>), Added<MeshMaterial3d<StandardMaterial>>>,
    highlights: Query<(), With<Highlight>>,
    parents: Query<&ChildOf>,
    white: Res<HighlightMaterial>,
) {
    for (e, mut material) in &mut meshes {
        if parents.iter_ancestors(e).any(|a| highlights.contains(a)) {
            material.0 = white.0.clone();
            commands.entity(e).try_insert(NotShadowCaster);
        }
    }
}
