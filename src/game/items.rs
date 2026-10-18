use std::f32::consts::FRAC_PI_2;

use bevy::{
    gltf::{GltfAssetLabel, GltfMaterialName},
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
    human_sync::{NetHuman, bone_at},
    netgame::{entities::Item, NetGame},
    smoothing::{Smoothing, Snapshots},
};
use crate::{character_model::HumanPose, menu::Screen};

const FIRST_POCKET: u32 = 2;
const BACK: f32 = 0.25;
const NOTES: [&str; 7] = ["one", "five", "ten", "twenty", "fifty", "hundred", "thousand"];
const BILL_GAP: f32 = 0.013_671_875;
const FAN: f32 = 0.031_25;
const FACE: &str = "skin";
const FACE_SHIFT: f32 = 0.1875;
const OTHER_FACE: i32 = 1;

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

#[derive(Resource)]
struct BillTextures(Vec<Handle<Image>>);

#[derive(Resource, Default)]
struct ItemMaterials(HashMap<(AssetId<StandardMaterial>, usize), Handle<StandardMaterial>>);

pub struct ItemsPlugin;

impl Plugin for ItemsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ItemMaterials>()
            .add_systems(Startup, |mut commands: Commands, assets: Res<AssetServer>| {
                commands.insert_resource(BillTextures(NOTES.iter().map(|n| assets.load(format!("textures/cash_{n}.png"))).collect()));
            })
            .add_systems(Update, sync_items.in_set(Smoothing::Sync).run_if(resource_exists::<NetGame>))
            .add_systems(Update, (stack_cash, mark_faces, show_faces, print_bills).chain().after(Smoothing::Sync).run_if(in_state(Screen::InGame)))
            .add_systems(Update, carry_on_back.after(Smoothing::Glide).before(Smoothing::View).run_if(in_state(Screen::InGame)));
    }
}

fn place(item: &Item) -> Transform {
    Transform::from_translation(item.track.pos).with_rotation(item.rot.conjugate())
}

fn stowed(item: &Item) -> bool {
    item.parent_human >= 0 && item.parent_slot > FIRST_POCKET
}

fn out_in_world(net: &NetGame, item: &Item) -> bool {
    let parent = usize::try_from(item.parent_item).ok().and_then(|p| net.items.get(p));

    !stowed(item) && parent.is_none_or(|p| !stowed(p))
}

fn on_back(net: &NetGame, item: &Item) -> Option<usize> {
    let human = usize::try_from(item.parent_human).ok().filter(|_| item.parent_slot >= FIRST_POCKET)?;

    net.humans.get(human).is_some_and(|h| h.active).then_some(human)
}

fn cash_stack(item: &Item) -> CashStack {
    let (bills, picked, notes) = match item.tail {
        ItemTail::Cash { bills, spread, codes } => (bills, spread, codes),
        _ => (0, 0, 0),
    };

    CashStack { bills, picked, notes, hand: (item.parent_human >= 0).then_some(item.parent_slot) }
}

fn phone_face(item: &Item) -> PhoneFace {
    PhoneFace(item.phone.2 == OTHER_FACE)
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
            face.set_if_neq(phone_face(item));
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
        let Some(ty) = types.0.get(kind as usize) else { continue };
        let model = assets.load(GltfAssetLabel::Scene(0).from_asset(format!("models/items/{}.glb", model_name(kind as usize, &ty.name))));
        let mut entity = commands.spawn((NetItem { index: i, kind }, place(item), Snapshots::new(item.track.tick, place(item)), Visibility::default(), DespawnOnExit(Screen::InGame)));

        match kind {
            ItemKind::CashWorld => entity.insert((cash_stack(item), ItemModel(model))),
            ItemKind::Phone => entity.insert((phone_face(item), WorldAssetRoot(model))),
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
            Some(0) => -FAN,
            Some(1) => FAN,
            _ => 0.0,
        };
        let turn = Quat::from_rotation_y(fan * FRAC_PI_2);
        let mut at = Transform::from_translation(Vec3::Y * stack.bills as f32 * BILL_GAP * 0.5);

        for k in 0..=stack.bills.max(0) {
            let note = (stack.notes >> (k * 3) & 7) as usize;
            commands.spawn((Bill(note), WorldAssetRoot(model.0.clone()), at, ChildOf(e)));

            at.translation -= at.rotation * Vec3::Y * BILL_GAP;

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
        if name.0 == FACE && parents.iter_ancestors(e).any(|a| phones.contains(a)) {
            commands.entity(e).insert(FaceSlot(material.0.clone()));
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

        let shift = |m: &mut StandardMaterial| m.uv_transform = Affine2::from_translation(Vec2::new(FACE_SHIFT, 0.0));
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

        *transform = Transform::from_translation(torso + z * BACK).with_rotation(Quat::from_mat3(&Mat3::from_cols(x, -z, y)));
    }
}
