use std::path::PathBuf;

use bevy::{
    asset::RenderAssetUsages,
    image::{CompressedImageFormats, ImageSampler, ImageType},
    prelude::*,
    render::render_resource::PrimitiveTopology,
};

use crate::models::{BONES, Cmc, Cmo, Corner, HEAD_BONE, apply_colours, bone_places};

const DEFAULT_GAME_DIR: &str = "S:/SteamLibrary/steamapps/common/Sub Rosa";
pub const HEADS: usize = 5;
pub const HAIRS: usize = 9;
const MODEL_SLOTS: usize = 0x18;
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

pub fn model_dir() -> PathBuf {
    PathBuf::from(std::env::var("SUBROSA_DIR").unwrap_or_else(|_| DEFAULT_GAME_DIR.to_string())).join("data").join("model")
}

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

#[derive(Component)]
struct HumanPart;

#[derive(Resource)]
pub struct HumanModels {
    skinned: Vec<Option<Cmc>>,
    heads: [Vec<Option<Cmo>>; 2],
    hairs: [Vec<Option<Cmo>>; 2],
    body_material: Handle<StandardMaterial>,
    head_material: Handle<StandardMaterial>,
}

pub struct HumanPlugin;

impl Plugin for HumanPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, load).add_systems(PostUpdate, build_humans.before(TransformSystems::Propagate));
    }
}

fn load(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>, mut images: ResMut<Assets<Image>>) {
    let dir = model_dir();
    let read = |name: String| std::fs::read(dir.join(&name)).ok();
    let mut skinned: Vec<Option<Cmc>> = (0..MODEL_SLOTS).map(|_| None).collect();

    for (slot, name) in MODEL_FILES {
        skinned[slot] = read(format!("{name}.cmc")).and_then(|b| Cmc::parse(&b));
    }

    let styles = |g: &str, kind: &str, count: usize| -> Vec<Option<Cmo>> { (1..=count).map(|k| read(format!("{g}{kind}{k}.cmo")).and_then(|b| Cmo::parse(&b))).collect() };
    let palette = |sampler: ImageSampler| {
        std::fs::read(dir.join("tex_1.png"))
            .ok()
            .and_then(|bytes| Image::from_buffer(&bytes, ImageType::Extension("png"), CompressedImageFormats::NONE, true, sampler, RenderAssetUsages::default()).ok())
            .unwrap_or_default()
    };

    let mut material = |image: Image| materials.add(StandardMaterial { base_color_texture: Some(images.add(image)), perceptual_roughness: 0.5, metallic: 0.0, ..default() });
    let body_material = material(palette(ImageSampler::linear()));
    let head_material = material(palette(ImageSampler::nearest()));

    commands.insert_resource(HumanModels {
        skinned,
        heads: [styles("f", "head", HEADS), styles("m", "head", HEADS)],
        hairs: [styles("f", "hair", HAIRS), styles("m", "hair", HAIRS)],
        body_material,
        head_material,
    });
}

fn body_slots(look: &HumanLook) -> [Option<usize>; 3] {
    let kind = if look.model == 2 || look.model == 3 { look.model as usize } else { (look.model == 1) as usize };
    let body = kind + if look.gender == 1 { 8 } else { 0 };
    let tie = (look.tie_colour > 0).then_some(0x10 + look.gender as usize * 2);
    let necklace = (look.necklace > 0).then_some(look.necklace as usize + look.gender as usize * 2 + 0x13);

    [Some(body), tie, necklace].map(|s| s.filter(|&s| s < MODEL_SLOTS))
}

fn mesh(corners: &[Corner]) -> Mesh {
    let tris: Vec<Corner> = corners.as_chunks::<3>().0.iter().flat_map(|t| [t[0], t[2], t[1]]).collect();

    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, tris.iter().map(|c| c.pos.to_array()).collect::<Vec<_>>())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, tris.iter().map(|c| c.normal.to_array()).collect::<Vec<_>>())
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, tris.iter().map(|c| [c.uv.x, 1.0 - c.uv.y]).collect::<Vec<_>>())
}

fn style(list: &[Option<Cmo>], k: u32) -> Option<&Cmo> {
    list.get(k as usize).and_then(Option::as_ref)
}

fn build_humans(
    mut commands: Commands,
    models: Option<Res<HumanModels>>,
    humans: Query<(Entity, &HumanLook, Option<&HumanPose>, Option<&Children>), Or<(Changed<HumanLook>, Changed<HumanPose>)>>,
    parts: Query<(), With<HumanPart>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(models) = models else { return };
    for (human, look, pose, children) in &humans {
        for &child in children.into_iter().flatten() {
            if parts.contains(child) {
                commands.entity(child).despawn();
            }
        }

        let rot = pose.copied().unwrap_or_default().0;
        let pos = bone_places(Vec3::ZERO, &rot);
        let tie = if look.tie_colour > 0 { look.tie_colour - 1 } else { 0 };
        let mut lowest = f32::MAX;

        for model in body_slots(look).into_iter().flatten().filter_map(|s| models.skinned[s].as_ref()) {
            let mut corners = model.posed_corners(&model.rest_pose(), &rot, &pos);

            apply_colours(&mut corners, [look.skin_colour, look.suit_colour, tie, 0]);

            lowest = corners.iter().fold(lowest, |m, c| m.min(c.pos.y));

            commands.spawn((Mesh3d(meshes.add(mesh(&corners))), MeshMaterial3d(models.body_material.clone()), Transform::default(), HumanPart, ChildOf(human)));
        }

        let g = (look.gender == 1) as usize;
        let head_at = Transform::from_translation(pos[HEAD_BONE] + rot[HEAD_BONE] * Vec3::Y * 0.125).with_rotation(rot[HEAD_BONE]);
        let colours = [look.skin_colour, look.hair_colour, look.eye_colour, 0];

        for model in [style(&models.heads[g], look.head), style(&models.hairs[g], look.hair)].into_iter().flatten() {
            let mut corners = model.head_corners(g == 0);

            apply_colours(&mut corners, colours);

            commands.spawn((Mesh3d(meshes.add(mesh(&corners))), MeshMaterial3d(models.head_material.clone()), head_at, HumanPart, ChildOf(human)));
        }

        commands.entity(human).insert(HumanExtent { lowest: if lowest == f32::MAX { 0.0 } else { lowest } });
    }
}
