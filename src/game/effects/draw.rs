use bevy::{
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

use super::{EffectAssets, Kind, Particles, empty_mesh};
use crate::game::camera::GameCamera;

const SMOKE_COLOUR: [f32; 3] = [1.0, 1.0, 0.75];

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

pub(super) fn draw_particles(
    particles: Res<Particles>,
    effect_assets: Res<EffectAssets>,
    camera: Option<Single<&Transform, With<GameCamera>>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(camera) = camera else { return };
    let (right, up, back) = (camera.right().as_vec3(), camera.up().as_vec3(), camera.back().as_vec3());
    let (mut spots, mut blood, mut glass) = (Batch::default(), Batch::default(), Batch::default());

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
}
