use bevy::math::{IVec3, Vec3};
use ferrosa_world::{
    area::{CUBE, MESH, TYPE_MASK},
    map::Map,
};

pub const BUILTIN: usize = 4;
pub const ASPHALT: usize = 1;
pub const ATLAS: usize = 3;
pub const MARKINGS: usize = 10;

const UP_AXES: [(Vec3, Vec3); 4] = [(Vec3::X, Vec3::NEG_Z), (Vec3::Z, Vec3::X), (Vec3::NEG_X, Vec3::Z), (Vec3::NEG_Z, Vec3::NEG_X)];

const CUBE_FACES: [[usize; 4]; 6] = [[3, 2, 1, 0], [7, 6, 2, 3], [4, 7, 3, 0], [5, 4, 0, 1], [6, 5, 1, 2], [4, 5, 6, 7]];
const CUBE_CORNERS: [Vec3; 8] = [
    Vec3::new(0.0, 0.0, 0.0),
    Vec3::new(1.0, 0.0, 0.0),
    Vec3::new(1.0, 0.0, 1.0),
    Vec3::new(0.0, 0.0, 1.0),
    Vec3::new(0.0, 1.0, 0.0),
    Vec3::new(1.0, 1.0, 0.0),
    Vec3::new(1.0, 1.0, 1.0),
    Vec3::new(0.0, 1.0, 1.0),
];
const CUBE_NORMALS: [Vec3; 6] = [Vec3::NEG_Y, Vec3::Z, Vec3::NEG_X, Vec3::NEG_Z, Vec3::X, Vec3::Y];
const CUBE_U: [Vec3; 6] = [Vec3::NEG_X, Vec3::X, Vec3::Z, Vec3::NEG_X, Vec3::NEG_Z, Vec3::X];
const CUBE_V: [Vec3; 6] = [Vec3::NEG_Z, Vec3::Y, Vec3::Y, Vec3::Y, Vec3::Y, Vec3::Z];

const MATERIAL_SCALE: f32 = 0.25;

#[derive(Default, Clone)]
pub struct Part {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

fn decode_slot(w: u16, cube: bool) -> (usize, usize) {
    let (mat, layer) = if (w >> 8) & 0x40 != 0 { (w & 0xff, (w >> 8) & 0x1f) } else { (w, 0) };
    if cube && (mat as i16) < 0 { (ATLAS, (mat & 0x3ff) as usize) } else { (mat as usize, layer as usize) }
}

pub struct Block {
    pub origin: IVec3,
    pub centre: Vec3,
    pub facades: Vec<Part>,
    pub interior: Vec<Part>,
    pub panes: Part,
}

#[derive(Default)]
pub struct Built {
    pub parts: Vec<Part>,
    pub blocks: Vec<Block>,
}

#[derive(Clone, Copy)]
enum Target {
    Part(usize),
    Facade(usize),
}

struct Builder<'a> {
    parts: Vec<Part>,
    atlas: &'a [(u16, [f32; 4])],
    marked: usize,
    block: Block,
    interior: bool,
}

impl Builder<'_> {
    fn resolve(&self, material: usize, layer: usize) -> Target {
        match material {
            ATLAS if !self.interior && layer < self.atlas.len() => Target::Facade(layer),
            ATLAS => Target::Part(self.atlas.get(layer).map_or(ATLAS, |e| e.0 as usize)),
            ASPHALT if (1..=MARKINGS).contains(&layer) => Target::Part(self.marked + layer - 1),
            _ => Target::Part(material),
        }
    }

    fn quad(&mut self, target: Target, corners: [(Vec3, Vec3, [f32; 2]); 4]) {
        let part = match target {
            Target::Facade(layer) => self.block.facades.get_mut(layer),
            Target::Part(k) if self.interior => self.block.interior.get_mut(k),
            Target::Part(k) => self.parts.get_mut(k),
        };

        let Some(part) = part else { return };
        let base = part.positions.len() as u32;

        for (p, n, uv) in corners {
            part.positions.push(p.to_array());
            part.normals.push(n.to_array());
            part.uvs.push([uv[0], 1.0 - uv[1]]);
        }

        part.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn mesh(&mut self, mesh: &ferrosa_world::mesh::BlockMesh, chunk: Vec3, local: Vec3, origin: Vec3, bs: f32, turns: usize, slot: &dyn Fn(usize) -> Option<(usize, usize)>) {
        let positions: Vec<Vec3> = mesh.verts.iter().map(|p| chunk + bs * *p + local + origin).collect();

        for (f, q) in mesh.quads.iter().enumerate() {
            let Some((material, layer)) = slot((mesh.face_codes[f] & 0xff) as usize) else { continue };
            let target = self.resolve(material, layer);

            let corners = q.map(|i| {
                let i = i as usize;
                let p = positions[i];
                let uv = if mesh.face_extra[f] & 1 != 0 {
                    let (u, w) = UP_AXES[turns];
                    let d = p - chunk - origin;
                    [d.dot(u) / bs, d.dot(w) / bs]
                } else {
                    mesh.uvs[i].to_array()
                };
                (p, mesh.normals[i], uv)
            });

            self.quad(target, corners);
        }
    }

    fn cube(&mut self, v: u32, slots: &[u16; 8], chunk: Vec3, local: Vec3, origin: Vec3, bs: f32) {
        let turns = ((v >> 24) & 3) as usize;
        for r in 0..6 {
            if v & (1 << r) == 0 {
                continue;
            }

            let (material, layer) = decode_slot(slots[r], true);
            let target = self.resolve(material, layer);
            let (n, u, w) = (CUBE_NORMALS[r], CUBE_U[r], CUBE_V[r]);
            let mut uvs = [[0.0f32; 2]; 4];
            let mut corners = [Vec3::ZERO; 4];

            for (k, &c) in CUBE_FACES[r].iter().enumerate() {
                let p = bs * CUBE_CORNERS[c] + local;
                uvs[k] = [p.dot(u) * MATERIAL_SCALE, p.dot(w) * MATERIAL_SCALE];
                corners[k] = chunk + p + origin;
            }

            if r == 5 {
                for _ in 0..turns {
                    uvs = [uvs[3], uvs[0], uvs[1], uvs[2]];
                }
            }

            self.quad(target, std::array::from_fn(|k| (corners[k], n, uvs[k])));
        }
    }

}

fn pane(part: &mut Part, corners: [Vec3; 4]) {
    let n = (corners[1] - corners[0]).cross(corners[2] - corners[0]).normalize_or_zero();
    let base = part.positions.len() as u32;

    for (k, p) in corners.into_iter().enumerate() {
        part.positions.push(p.to_array());
        part.normals.push(n.to_array());

        part.uvs.push([(k == 1 || k == 2) as u32 as f32, (k >= 2) as u32 as f32]);
    }

    part.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

pub fn panes(map: &Map, origin: IVec3) -> Part {
    let area = &map.level.area;
    let bs = area.block_size;
    let chunk = origin.as_vec3() * bs;
    let mut part = Part::default();
    let Some(record) = area.record(origin.x, origin.y, origin.z) else { return part };

    for (cell, v) in record.layer1.iter().enumerate() {
        if v & TYPE_MASK != MESH {
            continue;
        }

        let Some(mesh) = map.level.meshes.get(v & 0xffff) else { continue };
        let local = Vec3::new((cell & 7) as f32, (cell >> 6) as f32, ((cell >> 3) & 7) as f32) * bs;

        for (w, wall) in mesh.walls.iter().enumerate() {
            if ((v >> 16) & 0xff).checked_shr(w as u32).unwrap_or(0) & 1 == 0 {
                pane(&mut part, [3, 2, 1, 0].map(|k| chunk + bs * wall[k] + local + area.origin));
            }
        }
    }

    part
}

pub fn build(map: &Map) -> Built {
    let area = &map.level.area;
    let bs = area.block_size;
    let count = BUILTIN + map.level.texture_data.len();
    let empty = |n: usize| (0..n).map(|_| Part::default()).collect::<Vec<_>>();
    let new_block = |origin: IVec3, centre: Vec3| Block { origin, centre, facades: empty(map.level.atlas.len()), interior: empty(count + MARKINGS), panes: Part::default() };
    let mut b = Builder { parts: empty(count + MARKINGS), atlas: &map.level.atlas, marked: count, block: new_block(IVec3::ZERO, Vec3::ZERO), interior: false };
    let mut blocks = Vec::new();

    for (origin, record) in area.blocks() {
        let chunk = origin.as_vec3() * bs;
        b.block = new_block(origin, chunk + Vec3::splat(bs * 4.0) + area.origin);

        for cell in 0..512usize {
            let local = Vec3::new((cell & 7) as f32, (cell >> 6) as f32, ((cell >> 3) & 7) as f32) * bs;
            let slots = &record.cell_textures[cell];

            let v = record.layer0[cell];
            b.interior = false;

            if v & TYPE_MASK == CUBE {
                b.cube(v, slots, chunk, local, area.origin, bs);
            } else if v & TYPE_MASK == MESH
                && let Some(mesh) = map.level.meshes.get(v & 0xffff)
            {
                b.mesh(mesh, chunk, local, area.origin, bs, ((v >> 24) & 3) as usize, &|code| Some(decode_slot(*slots.get(code)?, false)));
            }

            let v = record.layer1[cell];
            b.interior = true;

            if v & TYPE_MASK == MESH
                && let Some(mesh) = map.level.meshes.get(v & 0xffff)
            {
                let bytes = &record.textures[cell];
                let word = |w: u16| if (w as i16) < 0 { (ATLAS, (w & 0x3ff) as usize) } else { ((w & 0xff) as usize, 0) };
                let slot = |code: usize| if code > 7 { Some(word(*slots.get(code - 8)?)) } else { Some((bytes[code] as usize, 0)) };
                b.mesh(mesh, chunk, local, area.origin, bs, ((v >> 24) & 3) as usize, &slot);
            }
        }

        b.block.panes = panes(map, origin);

        let next = new_block(IVec3::ZERO, Vec3::ZERO);

        blocks.push(std::mem::replace(&mut b.block, next));
    }

    Built { parts: b.parts, blocks }
}
