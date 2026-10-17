use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};
use bevy::{
    asset::RenderAssetUsages,
    camera::Exposure,
    gltf::{GltfAssetLabel, GltfMaterialName},
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use ferrosa_world::{
    city_objects::{GARAGE_DOOR, object_pose},
    streets::StreetMap,
};

use super::{Level, camera::EXPOSURE, netgame::NetGame};
use crate::{fonts::OVERPASS, menu::Screen};

const TRAFFIC_LIGHT: usize = 1;
const SIGN: usize = 4;
const LIGHT_MODEL: &str = "models/objects/object_1.glb";
const SIGN_MODEL: &str = "models/objects/object_4.glb";
const LAMP: &str = "lamp";
const SIGN_FACE: &str = "sign";
const LIGHTS: [&str; 3] = ["light_red", "light_amber", "light_green"];
const AMBER: i32 = 1;
const GREEN: i32 = 2;
const STREET_NAMES: [&str; 17] = [
    "First Street",
    "Second Street",
    "Third Street",
    "Fourth Street",
    "Fifth Street",
    "Sixth Street",
    "Seventh Street",
    "Eighth Street",
    "Ninth Street",
    "Abrash Ave",
    "Bunten Ave",
    "Carmack Ave",
    "David Ave",
    "Eugene Ave",
    "Flinn Ave",
    "Garriott Ave",
    "Holland Ave",
];
const SIGN_SIZE: (u32, u32) = (0x100, 0x40);
const SIGN_GROUND: [u8; 3] = [0, 0x40, 0];
const SIGN_TEXT: f32 = 52.0;

#[derive(Component)]
struct TrafficLight {
    intersection: usize,
    dir: usize,
}

#[derive(Component)]
struct StreetSign(usize);

#[derive(Component)]
struct LampSlot;

#[derive(Resource)]
struct StreetMaterials {
    lights: [Handle<StandardMaterial>; 3],
    signs: Vec<Handle<StandardMaterial>>,
}

pub struct StreetObjectsPlugin;

impl Plugin for StreetObjectsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::InGame), spawn_street_objects)
            .add_systems(Update, (mark_faces, light_lamps.run_if(resource_exists::<NetGame>)).chain().run_if(in_state(Screen::InGame)).run_if(resource_exists::<StreetMaterials>))
            .add_systems(OnExit(Screen::InGame), |mut commands: Commands| commands.remove_resource::<StreetMaterials>());
    }
}

fn street_id(streets: &StreetMap, street: i32) -> usize {
    let name = usize::try_from(street).ok().and_then(|s| streets.streets.get(s)).map_or("", |s| s.name);

    STREET_NAMES.iter().position(|&n| n == name).unwrap_or(0)
}

fn sign_street(streets: &StreetMap, intersection: usize, dir: usize) -> i32 {
    let Some(i) = streets.intersections.get(intersection) else { return 0 };
    let right = i.streets[(dir + 1) & 3];
    let street = if right == -1 { i.streets[(dir + 3) & 3] } else { right };

    street.max(0)
}

fn sign_image(font: &FontRef, name: &str) -> Image {
    let (w, h) = SIGN_SIZE;
    let mut rgba: Vec<u8> = (0..w * h).flat_map(|_| [SIGN_GROUND[0], SIGN_GROUND[1], SIGN_GROUND[2], 255]).collect();
    let scaled = font.as_scaled(PxScale::from(SIGN_TEXT));

    let mut glyphs = Vec::new();
    let (mut x, mut last) = (0.0, None);

    for c in name.chars() {
        let id = scaled.glyph_id(c);

        if let Some(prev) = last {
            x += scaled.kern(prev, id);
        }

        glyphs.push((id, x));
        x += scaled.h_advance(id);
        last = Some(id);
    }

    let left = (w as f32 - x) * 0.5;
    let baseline = (h as f32 + scaled.ascent() + scaled.descent()) * 0.5;

    for (id, at) in glyphs {
        let glyph = id.with_scale_and_position(scaled.scale(), point(left + at, baseline));
        let Some(outline) = font.outline_glyph(glyph) else { continue };
        let bounds = outline.px_bounds();

        outline.draw(|gx, gy, cover| {
            let (px, py) = (bounds.min.x as i32 + gx as i32, bounds.min.y as i32 + gy as i32);

            if px < 0 || py < 0 || px >= w as i32 || py >= h as i32 {
                return;
            }

            let i = ((py as u32 * w + px as u32) * 4) as usize;

            for (c, ground) in SIGN_GROUND.iter().enumerate() {
                rgba[i + c] = (*ground as f32 + (255.0 - *ground as f32) * cover.min(1.0)) as u8;
            }
        });
    }

    let mut image = Image::new(Extent3d { width: w, height: h, depth_or_array_layers: 1 }, TextureDimension::D2, rgba, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD);
    image.sampler = ImageSampler::linear();
    image
}

fn spawn_street_objects(mut commands: Commands, assets: Res<AssetServer>, level: Res<Level>, mut images: ResMut<Assets<Image>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let glow = Exposure { ev100: EXPOSURE }.exposure().recip();
    let lights = LIGHTS.map(|name| {
        let texture = |suffix: &str| Some(assets.load(format!("models/objects/textures/{name}{suffix}.png")));

        materials.add(StandardMaterial {
            base_color_texture: texture(""),
            metallic_roughness_texture: texture("_mr"),
            metallic: 1.0,
            perceptual_roughness: 1.0,
            emissive: LinearRgba::rgb(glow, glow, glow),
            emissive_texture: texture("_emissive"),
            ..default()
        })
    });

    let signs = match FontRef::try_from_slice(OVERPASS) {
        Ok(font) => STREET_NAMES
            .iter()
            .map(|name| materials.add(StandardMaterial { base_color_texture: Some(images.add(sign_image(&font, name))), metallic: 0.0, perceptual_roughness: 0.5, ..default() }))
            .collect(),
        Err(e) => {
            warn!("sign font: {e}");
            Vec::new()
        }
    };

    commands.insert_resource(StreetMaterials { lights, signs });

    let area = &level.map.level.area;
    let streets = &level.map.streets;
    let light_model = assets.load(GltfAssetLabel::Scene(0).from_asset(LIGHT_MODEL));
    let sign_model = assets.load(GltfAssetLabel::Scene(0).from_asset(SIGN_MODEL));

    for (base, record) in area.blocks() {
        for (k, &word) in record.object.iter().enumerate() {
            if word as i32 <= 0 {
                continue;
            }

            let cell = base + IVec3::new((k & 7) as i32, (k >> 6) as i32, ((k >> 3) & 7) as i32);
            let (kind, pos, rot) = object_pose(word, cell, area.block_size);

            if kind == GARAGE_DOOR as usize {
                continue;
            }

            let place = Transform::from_translation(pos + area.origin).with_rotation(Quat::from_mat3(&Mat3::from_cols(rot[0], rot[1], rot[2])));
            let intersection = ((word >> 12) & 0xfff) as usize;
            let dir = ((((word as i32) >> 10) & 3) as usize + 2) & 3;

            match kind {
                TRAFFIC_LIGHT => {
                    commands.spawn((TrafficLight { intersection, dir }, WorldAssetRoot(light_model.clone()), place, DespawnOnExit(Screen::InGame)));

                    let street = street_id(streets, sign_street(streets, intersection, dir));
                    commands.spawn((StreetSign(street), WorldAssetRoot(sign_model.clone()), place, DespawnOnExit(Screen::InGame)));
                }
                SIGN => {
                    commands.spawn((WorldAssetRoot(sign_model.clone()), place, DespawnOnExit(Screen::InGame)));
                }
                _ => {}
            }
        }
    }
}

fn mark_faces(
    mut commands: Commands,
    street: Res<StreetMaterials>,
    mut meshes: Query<(Entity, &GltfMaterialName, &mut MeshMaterial3d<StandardMaterial>), Added<GltfMaterialName>>,
    signs: Query<&StreetSign>,
    parents: Query<&ChildOf>,
) {
    for (e, name, mut material) in &mut meshes {
        match name.0.as_str() {
            LAMP => {
                commands.entity(e).insert(LampSlot);
            }
            SIGN_FACE => {
                let Some(&StreetSign(id)) = parents.iter_ancestors(e).find_map(|a| signs.get(a).ok()) else { continue };

                if let Some(m) = street.signs.get(id) {
                    material.0 = m.clone();
                }
            }
            _ => {}
        }
    }
}

fn light_lamps(
    net: Res<NetGame>,
    street: Res<StreetMaterials>,
    mut lamps: Query<(Entity, &mut MeshMaterial3d<StandardMaterial>), With<LampSlot>>,
    lights: Query<&TrafficLight>,
    parents: Query<&ChildOf>,
) {
    for (e, mut material) in &mut lamps {
        let Some(light) = parents.iter_ancestors(e).find_map(|a| lights.get(a).ok()) else { continue };
        let state = net.signals.get(light.intersection).map_or(0, |s| s[light.dir]);
        let lit = match state {
            AMBER => 1,
            GREEN => 2,
            _ => 0,
        };

        if material.0 != street.lights[lit] {
            material.0 = street.lights[lit].clone();
        }
    }
}
