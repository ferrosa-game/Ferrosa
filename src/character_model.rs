use bevy::{
    gltf::{GltfAssetLabel, GltfExtras, GltfMaterialName},
    camera::visibility::RenderLayers,
    math::Affine2,
    platform::collections::HashMap,
    prelude::*,
    world_serialization::WorldInstanceReady,
};

pub const BONES: usize = 16;
const BONE_NAMES: [&str; BONES] = [
    "Pelvis", "Stomach", "Torso", "Head", "ShoulderL", "ForearmL", "HandL", "ShoulderR", "ForearmR", "HandR", "ThighL", "ShinL", "FootL", "ThighR",
    "ShinR", "FootR",
];
pub const PARENTS: [usize; BONES] = [0, 0, 1, 2, 2, 4, 5, 2, 7, 8, 0, 10, 11, 0, 13, 14];
const PELVIS_DROP: Vec3 = Vec3::new(0.0, -0.125, 0.0);
const MODEL_FILES: [(usize, &str); 12] = [
    (0, "fcasual1"),
    (1, "fsuit1"),
    (2, "fsuit1"),
    (8, "mcasual1"),
    (9, "msuit1"),
    (10, "msuit1"),
    (0x10, "fnecktie1"),
    (0x12, "mnecktie1"),
    (0x14, "fnecklace1"),
    (0x15, "fnecklace4"),
    (0x16, "mnecklace1"),
    (0x17, "mnecklace4"),
];

const COLUMNS: f32 = 16.0;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(Transform, Visibility)]
pub struct HumanLook {
    pub gender: u32,
    pub head: u32,
    pub hair: u32,
    pub skin_colour: u32,
    pub hair_colour: u32,
    pub eye_colour: u32,
    pub model: u32,
    pub suit_colour: u32,
    pub tie_colour: u32,
    pub necklace: u32,
}

impl HumanLook {
    fn shape(&self) -> (u32, u32, bool, u32, u32, u32) {
        (self.gender, self.model, self.tie_colour > 0, self.necklace, self.head, self.hair)
    }

    fn column(&self, slot: &str) -> Option<u32> {
        Some(match slot {
            "skin" => self.skin_colour,
            "suit" => self.suit_colour,
            "tie" => self.tie_colour.saturating_sub(1),
            "hair" => self.hair_colour,
            "eyes" => self.eye_colour,
            _ => return None,
        })
    }
}

#[derive(Component)]
pub struct HideHead;

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct HumanPose(pub [Quat; BONES]);

impl Default for HumanPose {
    fn default() -> Self {
        Self([Quat::IDENTITY; BONES])
    }
}

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct HumanExtent {
    pub lowest: f32,
}

#[derive(Component, Clone, Debug)]
enum HumanPart {
    Body { head: Option<String>, hair: Option<String> },
    Extra,
    Head,
}

#[derive(Component, PartialEq)]
struct BuiltShape((u32, u32, bool, u32, u32, u32));

#[derive(Component)]
struct Joints([Entity; BONES]);

#[derive(Component)]
struct SlotMaterial {
    slot: String,
    base: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
struct ColouredMaterials(HashMap<(AssetId<StandardMaterial>, u32), Handle<StandardMaterial>>);

pub struct HumanPlugin;

impl Plugin for HumanPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ColouredMaterials>()
            .add_systems(Update, (build_humans, colour_humans, hide_heads).chain())
            .add_systems(PostUpdate, pose_humans.before(TransformSystems::Propagate));
    }
}

fn model(assets: &AssetServer, name: &str) -> WorldAssetRoot {
    WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(format!("models/{name}.glb"))))
}

fn body_models(look: &HumanLook) -> [Option<&'static str>; 3] {
    let kind = if look.model == 2 || look.model == 3 { look.model as usize } else { (look.model == 1) as usize };
    let body = kind + if look.gender == 1 { 8 } else { 0 };
    let tie = (look.tie_colour > 0).then_some(0x10 + look.gender as usize * 2);
    let necklace = (look.necklace > 0).then_some(look.necklace as usize + look.gender as usize * 2 + 0x13);
    [Some(body), tie, necklace].map(|s| s.filter(|&s| s < 24).and_then(|s| MODEL_FILES.iter().find(|f| f.0 == s).map(|f| f.1)))
}

fn build_humans(
    mut commands: Commands,
    assets: Res<AssetServer>,
    humans: Query<(Entity, &HumanLook, Option<&BuiltShape>, Option<&Children>), Changed<HumanLook>>,
    parts: Query<(), With<HumanPart>>,
) {
    for (human, look, built, children) in &humans {
        let shape = BuiltShape(look.shape());
        if built == Some(&shape) {
            continue;
        }
        for &child in children.into_iter().flatten() {
            if parts.contains(child) {
                commands.entity(child).despawn();
            }
        }
        commands.entity(human).insert(shape);
        let g = if look.gender == 1 { "m" } else { "f" };
        let head = (look.head < 5).then(|| format!("{g}head{}", look.head + 1));
        let hair = (look.hair < 9).then(|| format!("{g}hair{}", look.hair + 1));
        for (k, name) in body_models(look).into_iter().enumerate() {
            let Some(name) = name else { continue };
            let part = if k == 0 { HumanPart::Body { head: head.clone(), hair: hair.clone() } } else { HumanPart::Extra };
            commands.spawn((model(&assets, name), part, ChildOf(human))).observe(part_ready);
        }
    }
}

fn part_ready(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    assets: Res<AssetServer>,
    parts: Query<(&HumanPart, &ChildOf)>,
    children: Query<&Children>,
    names: Query<&Name>,
    extras: Query<&GltfExtras>,
    slots: Query<(&GltfMaterialName, &MeshMaterial3d<StandardMaterial>)>,
) {
    let part = ready.entity;
    let Ok((kind, &ChildOf(owner))) = parts.get(part) else { return };
    let mut joints: [Option<Entity>; BONES] = [None; BONES];
    let mut mount = None;
    for e in children.iter_descendants(part) {
        if let Ok(name) = names.get(e) {
            if let Some(k) = BONE_NAMES.iter().position(|b| *b == name.as_str()) {
                joints[k] = Some(e);
            } else if name.as_str() == "HeadMount" {
                mount = Some(e);
            }
        }
        if let Ok((slot, material)) = slots.get(e)
            && matches!(slot.0.as_str(), "skin" | "suit" | "tie" | "hair" | "eyes")
        {
            commands.entity(e).insert(SlotMaterial { slot: slot.0.clone(), base: material.0.clone() });
        }
        if let HumanPart::Body { .. } = kind
            && let Some(lowest) = extras.get(e).ok().and_then(|x| lowest(&x.value))
        {
            commands.entity(owner).insert(HumanExtent { lowest });
        }
    }
    if joints.iter().all(Option::is_some) {
        commands.entity(part).insert(Joints(joints.map(Option::unwrap)));
    }
    if let (HumanPart::Body { head, hair }, Some(mount)) = (kind, mount) {
        for name in [head, hair].into_iter().flatten() {
            commands.spawn((model(&assets, name), HumanPart::Head, ChildOf(mount))).observe(part_ready);
        }
    }
}

fn hide_heads(
    mut commands: Commands,
    parts: Query<(Entity, &HumanPart)>,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    meshes: Query<Has<RenderLayers>, With<Mesh3d>>,
    hiding: Query<(), With<HideHead>>,
) {
    for (e, part) in &parts {
        if !matches!(part, HumanPart::Head) {
            continue;
        }

        let hide = parents.iter_ancestors(e).any(|a| hiding.contains(a));

        for mesh in std::iter::once(e).chain(children.iter_descendants(e)) {
            match (meshes.get(mesh), hide) {
                (Ok(false), true) => {
                    commands.entity(mesh).insert(RenderLayers::layer(1));
                }
                (Ok(true), false) => {
                    commands.entity(mesh).remove::<RenderLayers>();
                }
                _ => {}
            }
        }
    }
}

fn lowest(extras: &str) -> Option<f32> {
    let rest = &extras[extras.find("\"lowest\"")? + "\"lowest\"".len()..];
    let rest = rest.trim_start().strip_prefix(':')?.trim_start();
    let end = rest.find(|c: char| !(c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E'))).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

fn colour_humans(
    humans: Query<(Entity, &HumanLook)>,
    changed: Query<(), Changed<HumanLook>>,
    mut meshes: Query<(Entity, &SlotMaterial, &mut MeshMaterial3d<StandardMaterial>)>,
    added: Query<(), Added<SlotMaterial>>,
    parents: Query<&ChildOf>,
    mut coloured: ResMut<ColouredMaterials>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, slot, mut material) in &mut meshes {
        let Some((human, look)) = parents.iter_ancestors(e).find_map(|a| humans.get(a).ok()) else { continue };
        if !added.contains(e) && !changed.contains(human) {
            continue;
        }
        let Some(column) = look.column(&slot.slot) else { continue };
        let key = (slot.base.id(), column);
        let handle = match coloured.0.get(&key) {
            Some(h) => h.clone(),
            None => {
                let Some(mut m) = materials.get(&slot.base).cloned() else { continue };
                m.uv_transform = Affine2::from_translation(Vec2::new(column as f32 / COLUMNS, 0.0));
                let h = materials.add(m);
                coloured.0.insert(key, h.clone());
                h
            }
        };
        if material.0 != handle {
            material.0 = handle;
        }
    }
}

fn pose_humans(
    humans: Query<Ref<HumanPose>>,
    parts: Query<(&ChildOf, Ref<Joints>)>,
    mut transforms: Query<&mut Transform>,
) {
    for (&ChildOf(human), joints) in &parts {
        let Ok(pose) = humans.get(human) else { continue };
        if !pose.is_changed() && !joints.is_added() {
            continue;
        }
        let rot = pose.0;
        for (i, &joint) in joints.0.iter().enumerate() {
            let Ok(mut t) = transforms.get_mut(joint) else { continue };
            if i == 0 {
                t.rotation = rot[0];
                t.translation = rot[0] * PELVIS_DROP;
            } else {
                t.rotation = rot[PARENTS[i]].inverse() * rot[i];
            }
        }
    }
}
