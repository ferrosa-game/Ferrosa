use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

use super::{items::NetItem, netgame::NetEvent, network::NetTick};
use crate::menu::Screen;

const FLASH_COLOUR: Color = Color::srgba(1.0, 0.75, 0.125, 0.25);
const GLOW_COLOUR: Color = Color::srgb(1.0, 0.75, 0.125);
const FACES: [[usize; 3]; 8] = [[0, 1, 2], [0, 2, 3], [0, 3, 4], [0, 4, 1], [2, 1, 5], [3, 2, 5], [4, 3, 5], [1, 4, 5]];

#[derive(Resource)]
struct FlashAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

#[derive(Component)]
struct MuzzleFlash {
    life: i32,
}

struct Waiting {
    index: usize,
    life: i32,
}

#[derive(Resource, Default)]
struct Pending(Vec<Waiting>);

pub struct MuzzlePlugin;

impl Plugin for MuzzlePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Pending>()
            .add_systems(OnEnter(Screen::InGame), spawn_flash_assets)
            .add_systems(
                FixedUpdate,
                (fade_flashes, flash_muzzles).chain().in_set(NetTick::Follow).run_if(in_state(Screen::InGame)).run_if(resource_exists::<FlashAssets>),
            )
            .add_systems(OnExit(Screen::InGame), |mut commands: Commands, mut pending: ResMut<Pending>| {
                commands.remove_resource::<FlashAssets>();
                pending.0.clear();
            });
    }
}

fn flash_size(life: i32) -> Vec3 {
    Vec3::splat((life as f32 / 64.0 + 1.0 / 8.0) * 0.8)
}

fn flash_glow(life: i32) -> f32 {
    life as f32 / 4.0 * 400_000.0
}

fn octahedron() -> Mesh {
    let corners = [Vec3::Y, Vec3::Z * 1.5, Vec3::NEG_X, Vec3::NEG_Z * 3.0, Vec3::X, Vec3::NEG_Y];
    let (mut positions, mut normals, mut uvs, mut indices) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());

    for face in FACES {
        let points = face.map(|k| corners[k]);
        let normal = (points[1] - points[0]).cross(points[2] - points[0]).normalize_or_zero();
        let base = positions.len() as u32;

        for p in points {
            positions.push(p.to_array());
            normals.push(normal.to_array());
            uvs.push([0.0f32; 2]);
        }

        indices.extend([base, base + 1, base + 2]);
    }

    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(indices))
}

fn spawn_flash_assets(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let shade = StandardMaterial { base_color: FLASH_COLOUR, unlit: true, alpha_mode: AlphaMode::Add, cull_mode: None, ..default() };
    let (mesh, material) = (meshes.add(octahedron()), materials.add(shade));

    commands.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(material.clone()),
        Transform::from_scale(Vec3::ZERO),
        NoFrustumCulling,
        NotShadowCaster,
        DespawnOnExit(Screen::InGame),
    ));
    commands.insert_resource(FlashAssets { mesh, material });
}

fn fade_flashes(mut commands: Commands, mut flashes: Query<(Entity, &mut MuzzleFlash, &mut Transform, &mut PointLight)>) {
    for (entity, mut flash, mut place, mut glow) in &mut flashes {
        flash.life -= 1;

        if flash.life <= 0 {
            commands.entity(entity).despawn();

            continue;
        }

        place.scale = flash_size(flash.life);
        glow.intensity = flash_glow(flash.life);
    }
}

fn flash_muzzles(
    mut commands: Commands,
    assets: Res<FlashAssets>,
    mut pending: ResMut<Pending>,
    mut happenings: MessageReader<NetEvent>,
    guns: Query<(Entity, &NetItem)>,
    children: Query<&Children>,
    names: Query<&Name>,
) {
    for happening in happenings.read() {
        let NetEvent::Gunshot { item, .. } = happening else { continue };
        let Ok(index) = usize::try_from(*item) else { continue };

        pending.0.push(Waiting { index, life: 4 });
    }

    pending.0.retain_mut(|waiting| {
        let gun = guns.iter().find(|(_, net)| net.index == waiting.index).map(|(gun, _)| gun);
        let muzzle = gun.and_then(|gun| children.iter_descendants(gun).find(|&e| names.get(e).is_ok_and(|name| name.as_str() == "muzzle_flash")));

        let Some(muzzle) = muzzle else {
            waiting.life -= 1;

            return waiting.life > 0;
        };

        commands.spawn((
            Mesh3d(assets.mesh.clone()),
            MeshMaterial3d(assets.material.clone()),
            Transform::from_scale(flash_size(waiting.life)),
            PointLight { color: GLOW_COLOUR, intensity: flash_glow(waiting.life), range: 6.0, shadow_maps_enabled: false, ..default() },
            MuzzleFlash { life: waiting.life },
            NoFrustumCulling,
            NotShadowCaster,
            ChildOf(muzzle),
            DespawnOnExit(Screen::InGame),
        ));

        false
    });
}
