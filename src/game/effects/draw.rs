use bevy::{
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

use super::{EffectAssets, FLASH_LIFE, Kind, Particle, Particles, empty_mesh};
use crate::game::{camera::GameCamera, items::NetItem};

const SMOKE_COLOUR: [f32; 3] = [1.0, 1.0, 0.75];
const FLASH_COLOUR: [f32; 4] = [1.0, 0.75, 0.125, 0.5];
const FLASH_BACK: f32 = 0.5;
const FLASH_REACH: f32 = 4.0;
pub(super) const FLASH_LIGHTS: usize = 8;
pub(super) const FLASH_LIGHT_COLOUR: Color = Color::srgb(1.0, 0.75, 0.125);
const FLASH_LUMENS: f32 = 400_000.0;
pub(super) const FLASH_RANGE: f32 = 6.0;

#[derive(Component)]
pub(super) struct FlashLight;

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
            *mesh = empty_mesh(PrimitiveTopology::TriangleList, 3);
            return;
        }

        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colours);
        mesh.insert_indices(Indices::U32(self.indices));
    }
}

fn flash_size(p: &Particle) -> f32 {
    p.life as f32 / 64.0 + 1.0 / 8.0
}

fn flash_glow(p: &Particle, gun: &Transform) -> Vec3 {
    gun.transform_point(p.pos) + gun.rotation * Vec3::NEG_Z * flash_size(p)
}

fn flash(batch: &mut Batch, p: &Particle, gun: &Transform, camera: &Transform) {
    let s = flash_size(p);
    let muzzle = gun.transform_point(p.pos);
    let along = gun.rotation * Vec3::NEG_Z;
    let back = camera.back().as_vec3();
    let side = along.cross(back).normalize_or_zero() * s;
    let (near, far) = (muzzle - along * (s * FLASH_BACK), muzzle + along * (s * FLASH_REACH));
    let (right, up) = (camera.right().as_vec3() * s, camera.up().as_vec3() * s);
    let glow = flash_glow(p, gun);

    batch.quad([far - side, far + side, near + side, near - side], back, FLASH_COLOUR);
    batch.quad([glow - right + up, glow + right + up, glow + right - up, glow - right - up], back, FLASH_COLOUR);
}

pub(super) fn draw_particles(
    particles: Res<Particles>,
    effect_assets: Res<EffectAssets>,
    camera: Option<Single<&Transform, With<GameCamera>>>,
    guns: Query<(&NetItem, &Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(camera) = camera else { return };
    let (right, up, back) = (camera.right().as_vec3(), camera.up().as_vec3(), camera.back().as_vec3());
    let (mut spots, mut blood, mut glass, mut flashes) = (Batch::default(), Batch::default(), Batch::default(), Batch::default());

    for p in &particles.list {
        let fade = (p.life as f32 / 32.0).min(1.0);
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
            Kind::Flash => {
                if let Some((_, gun)) = guns.iter().find(|(item, _)| Some(item.index) == p.gun) {
                    flash(&mut flashes, p, gun, &camera);
                }
            }
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

    if let Some(mut mesh) = meshes.get_mut(&effect_assets.flash) {
        flashes.write(&mut mesh);
    }
}

pub(super) fn light_flashes(
    particles: Res<Particles>,
    guns: Query<(&NetItem, &Transform)>,
    mut lights: Query<(&mut PointLight, &mut Transform, &mut Visibility), (With<FlashLight>, Without<NetItem>)>,
) {
    let mut lit = particles.list.iter().filter(|p| p.kind == Kind::Flash).filter_map(|p| Some((p, guns.iter().find(|(item, _)| Some(item.index) == p.gun)?.1)));

    for (mut light, mut place, mut visibility) in &mut lights {
        let Some((p, gun)) = lit.next() else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };

        light.intensity = FLASH_LUMENS * p.life as f32 / FLASH_LIFE as f32;
        place.translation = flash_glow(p, gun);
        visibility.set_if_neq(Visibility::Visible);
    }
}
