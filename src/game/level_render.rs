use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::VisibilityRange,
    image::ImageSampler,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

use ferrosa_world::{collide::calculate_face_normal, track::Track};

use super::{
    Level,
    blocks::Part,
    loading::LevelGeometry,
    textures::{self, Pixels},
};
use crate::menu::Screen;

const WATER: Color = Color::srgba(0.1, 0.25, 0.35, 0.85);
const GLASS: Color = Color::srgba(0.6, 0.7, 0.75, 0.25);
const GRASS: Color = Color::srgb(0.25, 0.33, 0.15);
const DARK: Color = Color::srgb(0.06, 0.06, 0.06);
const MISSING: Color = Color::srgb(1.0, 0.25, 0.5);

#[derive(Component)]
pub struct BlockPanes {
    pub origin: IVec3,
    pub centre: Vec3,
}

#[derive(Resource)]
pub struct AsphaltMaterial(pub Handle<StandardMaterial>);

#[derive(Resource)]
struct BlockMaterials {
    blocks: Vec<Handle<StandardMaterial>>,
    facades: Vec<Handle<StandardMaterial>>,
    glass: Handle<StandardMaterial>,
}

impl BlockMaterials {
    fn block(&self, k: usize) -> Handle<StandardMaterial> {
        self.blocks[k.min(self.blocks.len() - 1)].clone()
    }
}

pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::InGame), (make_materials, spawn_blocks, spawn_track, spawn_terrain, spawn_water).chain())
            .add_systems(OnExit(Screen::InGame), |mut commands: Commands| {
                commands.remove_resource::<Level>();
                commands.remove_resource::<BlockMaterials>();
                commands.remove_resource::<AsphaltMaterial>();
            });
    }
}

fn mesh(positions: Vec<[f32; 3]>, normals: Vec<[f32; 3]>, uvs: Option<Vec<[f32; 2]>>, indices: Vec<u32>) -> Mesh {
    let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals);

    if let Some(uvs) = uvs {
        m.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    }

    m.insert_indices(Indices::U32(indices));
    m
}

pub fn centred_mesh(part: Part, origin: Vec3) -> Mesh {
    let positions = part.positions.iter().map(|p| (Vec3::from_array(*p) - origin).to_array()).collect();

    mesh(positions, part.normals, Some(part.uvs), part.indices)
}

fn part_mesh(meshes: &mut Assets<Mesh>, part: Part, origin: Vec3) -> Option<Mesh3d> {
    if part.indices.is_empty() {
        return None;
    }

    Some(Mesh3d(meshes.add(centred_mesh(part, origin))))
}

fn surface(base_color: Color, base_color_texture: Option<Handle<Image>>) -> StandardMaterial {
    StandardMaterial { base_color, base_color_texture, perceptual_roughness: 0.9, reflectance: 0.2, cull_mode: None, ..default() }
}

fn make_materials(mut commands: Commands, level: Res<Level>, geometry: Res<LevelGeometry>, mut images: ResMut<Assets<Image>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let decoded: Vec<Option<Pixels>> =
        geometry.images.iter().enumerate().map(|(k, bytes)| if k == 0 { Some(textures::concrete()) } else { bytes.as_deref().and_then(Pixels::decode_png) }).collect();

    let asphalt = decoded[1].as_ref();
    let marked = (1..=10).map(|layer| (1, asphalt.map(|a| textures::road_markings(a, layer))));
    let plain = decoded.iter().cloned().enumerate();

    let marked_from = decoded.len();
    let blocks: Vec<Handle<StandardMaterial>> = plain
        .chain(marked)
        .enumerate()
        .map(|(n, (k, pixels))| {
            let image = pixels.map(|p| {
                let width = p.width;
                let mut image = p.into_image();

                if n >= marked_from
                    && let ImageSampler::Descriptor(sampler) = &mut image.sampler
                {
                    sampler.lod_max_clamp = (width / 32).max(1).ilog2() as f32;
                }

                images.add(image)
            });
            let colour = match (k, &image) {
                (_, Some(_)) => Color::WHITE,
                (2, _) => DARK,
                _ => MISSING,
            };

            materials.add(surface(colour, image))
        })
        .collect();

    let facades = level
        .map
        .level
        .atlas
        .iter()
        .map(|&(wall, window)| {
            let (colour, finish) = textures::window_facade(decoded.get(wall as usize).and_then(Option::as_ref), window);

            materials.add(StandardMaterial {
                base_color_texture: Some(images.add(colour.into_image())),
                metallic_roughness_texture: Some(images.add(finish.into_linear_image())),
                metallic: 1.0,
                perceptual_roughness: 1.0,
                cull_mode: None,
                ..default()
            })
        })
        .collect();

    let glass = materials.add(StandardMaterial {
        base_color: GLASS,
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.05,
        reflectance: 1.0,
        double_sided: true,
        cull_mode: None,
        ..default()
    });

    commands.insert_resource(AsphaltMaterial(blocks[1].clone()));
    commands.insert_resource(BlockMaterials { blocks, facades, glass });
}

fn track_part(track: &Track, kind: Option<i32>, part: usize, lift: f32) -> Option<Mesh> {
    let (mut positions, mut normals, mut uvs) = (Vec::new(), Vec::new(), Vec::new());

    for (i, m) in track.meshes.iter().enumerate() {
        if kind.is_some_and(|k| k != m.kind) {
            continue;
        }

        let corners = if i & 1 != 0 { [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0]] } else { [[0.0, 1.0], [1.0, 0.0], [1.0, 1.0]] };

        for j in m.pieces[part]..m.pieces[part + 4] {
            let Some(tri) = usize::try_from(j).ok().and_then(|j| m.tris.get(j)) else { continue };
            let p = tri.map(|v| usize::try_from(v).ok().and_then(|v| m.verts.get(v)).copied().unwrap_or_default());
            let n = calculate_face_normal(p[0], p[1], p[2]);

            for (corner, uv) in p.iter().zip(corners) {
                positions.push([corner.x, corner.y + lift, corner.z]);
                normals.push(n.to_array());
                uvs.push([uv[0], 1.0 - uv[1]]);
            }
        }
    }

    if positions.is_empty() {
        return None;
    }

    let indices = (0..positions.len() as u32).collect();

    Some(mesh(positions, normals, Some(uvs), indices))
}

fn spawn_track(mut commands: Commands, level: Res<Level>, materials: Res<BlockMaterials>, mut meshes: ResMut<Assets<Mesh>>) {
    if level.map.map_name != "test2" {
        return;
    }

    let track = Track::race_track();

    for (kind, part, lift, material) in [(Some(0), 2, 0.0, 0), (None, 0, 0.125, 2), (Some(1), 2, 0.0, 1)] {
        if let Some(m) = track_part(&track, kind, part, lift) {
            commands.spawn((Mesh3d(meshes.add(m)), MeshMaterial3d(materials.block(material)), DespawnOnExit(Screen::InGame)));
        }
    }
}

fn spawn_blocks(mut commands: Commands, mut geometry: ResMut<LevelGeometry>, materials: Res<BlockMaterials>, mut meshes: ResMut<Assets<Mesh>>) {
    let built = std::mem::take(&mut geometry.blocks);

    for (k, part) in built.parts.into_iter().enumerate() {
        if let Some(m) = part_mesh(&mut meshes, part, Vec3::ZERO) {
            commands.spawn((m, MeshMaterial3d(materials.block(k)), DespawnOnExit(Screen::InGame)));
        }
    }

    let fade = 192.0 - 32.0..192.0 + 32.0;
    let near = VisibilityRange { start_margin: 0.0..0.0, end_margin: fade.clone(), use_aabb: false };
    let far = VisibilityRange { start_margin: fade, end_margin: f32::MAX..f32::MAX, use_aabb: false };

    for block in built.blocks {
        let at = (Transform::from_translation(block.centre), DespawnOnExit(Screen::InGame));

        for (layer, part) in block.facades.into_iter().enumerate() {
            if let Some(m) = part_mesh(&mut meshes, part, block.centre) {
                commands.spawn((m, MeshMaterial3d(materials.facades[layer].clone()), far.clone(), at.clone()));
            }
        }

        for (k, part) in block.interior.into_iter().enumerate() {
            if let Some(m) = part_mesh(&mut meshes, part, block.centre) {
                commands.spawn((m, MeshMaterial3d(materials.block(k)), near.clone(), at.clone()));
            }
        }

        if let Some(m) = part_mesh(&mut meshes, block.panes, block.centre) {
            let panes = BlockPanes { origin: block.origin, centre: block.centre };
            commands.spawn((m, panes, MeshMaterial3d(materials.glass.clone()), NotShadowCaster, near.clone(), at.clone()));
        }
    }
}

fn spawn_terrain(mut commands: Commands, mut geometry: ResMut<LevelGeometry>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let grass = materials.add(surface(GRASS, None));

    for tile in std::mem::take(&mut geometry.terrain) {
        let m = meshes.add(mesh(tile.positions, tile.normals, None, tile.indices));
        commands.spawn((Mesh3d(m), MeshMaterial3d(grass.clone()), DespawnOnExit(Screen::InGame)));
    }

    commands.remove_resource::<LevelGeometry>();
}

fn spawn_water(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let corners = [[-4069.0, -4069.0], [2048.0, -4069.0], [2048.0, 2048.0], [-4069.0, 2048.0]];
    let plane = mesh(corners.map(|[x, z]| [x, 23.0, z]).to_vec(), vec![[0.0, 1.0, 0.0]; 4], None, vec![0, 2, 1, 0, 3, 2]);

    let water = StandardMaterial { base_color: WATER, alpha_mode: AlphaMode::Blend, perceptual_roughness: 0.1, cull_mode: None, ..default() };

    commands.spawn((Mesh3d(meshes.add(plane)), MeshMaterial3d(materials.add(water)), DespawnOnExit(Screen::InGame)));
}
