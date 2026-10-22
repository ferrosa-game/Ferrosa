use ferrosa_world::ground::Ground;

#[derive(Default)]
pub struct Tile {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

fn height(ground: &Ground, x: i32, z: i32) -> f32 {
    let (sx, sz) = (x + 4096, z + 4096);

    if !(0..16384).contains(&sx) || !(0..16384).contains(&sz) {
        return 0.0;
    }

    ground.vertex(sx, sz)
}

fn grid(ground: &Ground, x0: i32, z0: i32, size: i32, step: i32, skip: &dyn Fn(i32, i32) -> bool) -> Tile {
    let n = size / step;
    let side = (n + 1) as u32;
    let mut t = Tile::default();

    for j in 0..=n {
        for i in 0..=n {
            let (x, z) = (x0 + i * step, z0 + j * step);
            let h = height(ground, x, z);
            let dx = height(ground, x - step, z) - height(ground, x + step, z);
            let dz = height(ground, x, z - step) - height(ground, x, z + step);
            let normal = bevy::math::Vec3::new(dx, 2.0 * step as f32, dz).normalize_or_zero();
            t.positions.push([x as f32, h, z as f32]);
            t.normals.push(normal.to_array());
        }
    }

    for j in 0..n {
        for i in 0..n {
            if skip(x0 + i * step, z0 + j * step) {
                continue;
            }

            let v = j as u32 * side + i as u32;
            t.indices.extend_from_slice(&[v, v + side, v + side + 1, v, v + side + 1, v + 1]);
        }
    }

    t
}

pub fn build(ground: &Ground, near: (i32, i32, i32, i32)) -> Vec<Tile> {
    let snap = |v: i32, up: bool| if up { (v + 256 - 1).div_euclid(256) * 256 } else { v.div_euclid(256) * 256 };
    let (nx0, nz0) = (snap(near.0 - 512, false), snap(near.1 - 512, false));
    let (nx1, nz1) = (snap(near.2 + 512, true), snap(near.3 + 512, true));
    let inside = |x: i32, z: i32| (nx0..nx1).contains(&x) && (nz0..nz1).contains(&z);

    let mut tiles = Vec::new();
    for z0 in (nz0..nz1).step_by(256_usize) {
        for x0 in (nx0..nx1).step_by(256_usize) {
            let cleared = |x: i32, z: i32| !ground.cell_ok(x + 4096, z + 4096);

            tiles.push(grid(ground, x0, z0, 256, 2, &cleared));
        }
    }

    for z0 in (-4096..-4096 + 16384).step_by(2048_usize) {
        for x0 in (-4096..-4096 + 16384).step_by(2048_usize) {
            let covered = |x: i32, z: i32| inside(x, z) && inside(x + 32 - 1, z + 32 - 1);
            
            tiles.push(grid(ground, x0, z0, 2048, 32, &covered));
        }
    }

    tiles.retain(|t| !t.indices.is_empty());
    tiles
}
