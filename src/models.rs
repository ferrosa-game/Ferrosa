use bevy::math::{Quat, Vec2, Vec3};

pub const BONES: usize = 16;
pub const PARENTS: [usize; BONES] = [0, 0, 1, 2, 2, 4, 5, 2, 7, 8, 0, 10, 11, 0, 13, 14];
pub const HEAD_BONE: usize = 3;
const SKELETON: [(Vec3, Vec3); BONES] = [
    (Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, -0.125, 0.0)),
    (Vec3::new(0.0, 0.25, 0.0), Vec3::new(0.0, 0.125, 0.0)),
    (Vec3::new(0.0, 0.25, 0.0), Vec3::new(0.0, 0.125, 0.0)),
    (Vec3::new(0.0, 0.28125, 0.0), Vec3::new(0.0, 0.1875, 0.0)),
    (Vec3::new(-0.1875, -13.0 / 128.0, 0.0), Vec3::new(-0.1875, 0.0625, 0.0)),
    (Vec3::new(0.0, -21.0 / 64.0, 0.0), Vec3::new(0.0, -21.0 / 128.0, 0.0)),
    (Vec3::new(0.0, -29.0 / 128.0, 0.0), Vec3::new(0.0, -21.0 / 128.0, 0.0)),
    (Vec3::new(0.1875, -13.0 / 128.0, 0.0), Vec3::new(0.1875, 0.0625, 0.0)),
    (Vec3::new(0.0, -21.0 / 64.0, 0.0), Vec3::new(0.0, -21.0 / 128.0, 0.0)),
    (Vec3::new(0.0, -29.0 / 128.0, 0.0), Vec3::new(0.0, -21.0 / 128.0, 0.0)),
    (Vec3::new(-0.09375, -0.25, 0.0), Vec3::new(-0.09375, 0.0, 0.0)),
    (Vec3::new(0.0, -0.5, 0.0), Vec3::new(0.0, -0.25, 0.0)),
    (Vec3::new(0.0, -0.25, -0.0625), Vec3::new(0.0, -0.25, 0.0)),
    (Vec3::new(0.09375, -0.25, 0.0), Vec3::new(0.09375, 0.0, 0.0)),
    (Vec3::new(0.0, -0.5, 0.0), Vec3::new(0.0, -0.25, 0.0)),
    (Vec3::new(0.0, -0.25, -0.0625), Vec3::new(0.0, -0.25, 0.0)),
];

pub fn bone_places(root: Vec3, rot: &[Quat; BONES]) -> [Vec3; BONES] {
    let mut pos = [Vec3::ZERO; BONES];
    pos[0] = root + rot[0] * Vec3::NEG_Y * 0.125;
    for i in 1..BONES {
        let p = PARENTS[i];
        let mut d = SKELETON[p].1 - SKELETON[p].0;
        if i == 12 || i == 15 {
            d.z -= 0.0625;
            d.y -= 0.0625 + 0.03125;
        }
        pos[i] = rot[p] * (SKELETON[i].1 - d) + pos[p];
    }
    pos
}

#[derive(Clone, Copy, Debug)]
pub struct Corner {
    pub pos: Vec3,
    pub uv: Vec2,
    pub normal: Vec3,
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn i32(&mut self) -> Option<i32> {
        let (b, rest) = self.0.split_first_chunk::<4>()?;
        self.0 = rest;
        Some(i32::from_le_bytes(*b))
    }

    fn f32(&mut self) -> Option<f32> {
        self.i32().map(|v| f32::from_bits(v as u32))
    }

    fn vec3(&mut self) -> Option<Vec3> {
        Some(Vec3::new(self.f32()?, self.f32()?, self.f32()?))
    }
}
fn face_normal(a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    (c - a).cross(b - a).normalize_or_zero()
}

pub struct Cmc {
    bones: Vec<Vec3>,
    vertices: Vec<(Vec2, Vec<(Vec3, f32)>)>,
    faces: Vec<[i32; 3]>,
}

impl Cmc {
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let mut r = Reader(bytes.get(4..)?);
        if r.i32()? != 2 {
            return None;
        }
        let bone_count = r.i32()?.max(0) as usize;
        let bones = (0..bone_count).map(|_| r.vec3().map(|v| v * 1.125)).collect::<Option<Vec<_>>>()?;
        let vertex_count = r.i32()?.max(0) as usize;
        let mut vertices = Vec::with_capacity(vertex_count);
        for _ in 0..vertex_count {
            r.vec3()?;
            let pulls = (0..bone_count).map(|_| Some((r.vec3()? * 1.125, r.f32()?))).collect::<Option<Vec<_>>>()?;
            vertices.push((Vec2::new(r.f32()?, r.f32()?), pulls));
        }
        let face_count = r.i32()?.max(0) as usize;
        let faces = (0..face_count).map(|_| Some([r.i32()?, r.i32()?, r.i32()?])).collect::<Option<Vec<_>>>()?;
        Some(Self { bones, vertices, faces })
    }

    pub fn rest_pose(&self) -> [Vec3; BONES] {
        let mut rest = [Vec3::ZERO; BONES];
        rest[0] = Vec3::new(0.0, -0.1875, 0.0625);
        for i in 1..BONES {
            let mut v = self.bones.get(i).copied().unwrap_or_default();
            if matches!(i, 5 | 6 | 8 | 9) {
                v = v.normalize_or_zero() * 0.328125;
            }
            if i == 4 || i == 7 {
                v.x = if i == 4 { -0.1875 } else { 0.1875 };
                v.y = 0.328125;
            }
            rest[i] = if i == 4 || i == 7 { Vec3::new(0.0, 0.1640625, 0.0) } else { rest[PARENTS[i]] + v };
        }
        rest
    }

    pub fn posed_corners(&self, rest: &[Vec3; BONES], rot: &[Quat; BONES], pos: &[Vec3; BONES]) -> Vec<Corner> {
        let placed: Vec<Vec3> = self
            .vertices
            .iter()
            .map(|(_, pulls)| {
                let kept: Vec<(usize, Vec3, f32)> = pulls.iter().enumerate().filter(|(_, p)| !(p.1 <= 0.0)).take(4).map(|(j, &(o, w))| (j, o, w)).collect();
                let sum: f32 = kept.iter().map(|k| k.2).sum();
                let at_rest = kept.iter().fold(Vec3::ZERO, |p, &(j, o, w)| p + (o + rest.get(j).copied().unwrap_or_default()) * (w / sum));
                kept.iter().fold(Vec3::ZERO, |p, &(j, _, w)| p + (rot[j] * (at_rest - rest[j]) + pos[j]) * (w / sum))
            })
            .collect();
        self.faces
            .iter()
            .filter_map(|f| {
                let c = f.map(|i| usize::try_from(i).ok().filter(|&i| i < placed.len()));
                let c = [c[0]?, c[1]?, c[2]?];
                let normal = face_normal(placed[c[0]], placed[c[1]], placed[c[2]]);
                Some(c.map(|i| Corner { pos: placed[i], uv: body_slots(self.vertices[i].0), normal }))
            })
            .flatten()
            .collect()
    }
}

fn body_slots(mut uv: Vec2) -> Vec2 {
    let band = |v: f32, lo: f32, hi: f32| !(v < lo) && hi > v;
    if band(uv.y, 0.9375, 1.0) {
        uv.x = 16.5;
    }
    if band(uv.y, 0.625, 0.6875) {
        uv.x = 17.5;
    }
    if band(uv.y, 0.375, 0.4375) && !(0.3125 <= uv.x) {
        uv.x = 17.5;
    }
    if band(uv.y, 0.5, 0.5625) {
        uv.x = 18.5;
    }
    uv
}

pub struct Cmo {
    version: i32,
    positions: Vec<Vec3>,
    uvs: Vec<Vec2>,
    faces: Vec<(Vec<i32>, i32)>,
}

impl Cmo {
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let (magic, rest) = bytes.split_first_chunk::<4>()?;
        if magic != b"CMod" {
            return None;
        }

        let mut r = Reader(rest);

        let version = r.i32()?;
        if !(1..=3).contains(&version) {
            return None;
        }

        let count = r.i32()?.max(0) as usize;
        let mut positions = Vec::with_capacity(count);
        let mut uvs = Vec::with_capacity(count);

        for _ in 0..count {
            positions.push(r.vec3()?);
            uvs.push(if version > 2 { Vec2::new(r.f32()?, r.f32()?) } else { Vec2::ZERO });
            r.i32()?;
        }

        let face_count = r.i32()?.max(0) as usize;
        let mut faces = Vec::with_capacity(face_count);

        for _ in 0..face_count {
            let n = r.i32()?.max(0) as usize;
            let indices = (0..n).map(|_| r.i32()).collect::<Option<Vec<_>>>()?;
            r.i32()?;
            let skip = if version > 1 { r.i32()? } else { 0 };
            faces.push((indices, skip));
        }
        Some(Self { version, positions, uvs, faces })
    }

    pub fn head_corners(&self, female: bool) -> Vec<Corner> {
        let place = |p: Vec3| {
            let y = p.y - 0.0234375 - if female { 0.03125 } else { 0.0 };
            Vec3::new(-p.x, y, -p.z)
        };

        let positions: Vec<Vec3> = self.positions.iter().map(|&p| place(p)).collect();

        let corner = |i: i32, default: Vec2| -> Option<(Vec3, Vec2)> {
            let i = usize::try_from(i).ok()?;
            Some((*positions.get(i)?, if self.version > 2 { self.uvs[i] } else { default }))
        };

        self.faces
            .iter()
            .flat_map(|(f, skip)| match (f.as_slice(), *skip) {
                (&[a, b, c], _) => vec![[a, b, c].map(|i| corner(i, Vec2::X))],
                (&[a, b, c, d], 0) => vec![[a, b, c].map(|i| corner(i, Vec2::ZERO)), [a, c, d].map(|i| corner(i, Vec2::ZERO))],
                _ => Vec::new(),
            })
            .filter_map(|t| Some([t[0]?, t[1]?, t[2]?]))
            .flat_map(|t| {
                let normal = face_normal(t[0].0, t[1].0, t[2].0);
                head_slots(t.map(|(pos, uv)| Corner { pos, uv, normal }))
            })
            .collect()
    }
}

fn head_slots(mut t: [Corner; 3]) -> [Corner; 3] {
    let band = |v: f32, lo: f32, hi: f32| !(v < lo) && !(hi <= v);
    let sweeps: [(f32, f32, Option<f32>, f32); 4] = [(0.9375, 1.0, None, 16.5), (0.8125, 0.875, Some(0.5), 16.5), (0.75, 0.8125, None, 17.5), (0.875, 0.9375, Some(0.5), 18.5)];

    for (lo, hi, below, slot) in sweeps {
        for c in t.iter_mut() {
            if band(c.uv.y, lo, hi) && below.is_none_or(|b| !(b <= c.uv.x)) {
                c.uv.x = slot;
            }
        }
    }

    t
}

pub fn apply_colours(corners: &mut [Corner], colours: [u32; 4]) {
    for c in corners {
        if (16.0..20.0).contains(&c.uv.x) {
            c.uv.x = (colours[c.uv.x as usize - 16] as f32 + 0.5) * 0.0625;
        }
    }
}
