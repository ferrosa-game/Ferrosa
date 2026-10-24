use bevy::prelude::*;
use ferrosa_world::{
    area::{MESH, TYPE_MASK},
    map::Map,
};

use super::{Kind, Particle, Particles, RESTING_LIFT, bounce, byte, frame_across, random_vec3, trace};
use crate::game::Level;

pub(super) fn shatter_window(particles: &mut Particles, c: [Vec3; 4], hit: Vec3, hit_vel: Vec3) {
    let n = (c[1] - c[0]).cross(c[2] - c[0]).normalize_or_zero();
    let (side, up) = frame_across(n);

    let steps = 8.0;
    let (left, right) = ((c[3] - c[0]) / steps, (c[2] - c[1]) / steps);
    let (mut start, mut end) = (c[0], c[1]);

    for _ in 0..9 {
        let step = (end - start) / steps;
        let mut p = start;

        for _ in 0..9 {
            let near = (1.0 - p.distance(hit) * 4.0).max(1.0 / 32.0);
            let vel = (p - hit) / 32.0 + random_vec3(1.0 / 120.0, 1.0 / 480.0) + hit_vel * near / 16.0;
            let mut s = Particle::new(Kind::Shard, p, vel);

            s.size = 1.0 / 16.0;
            s.frame = [side, up, n];
            s.spin_axis = Vec3::new(byte() / 512.0 - 0.25, byte() / 512.0 - 0.25, byte() / 512.0 - 0.25).normalize_or_zero();
            s.spin = (byte() / 512.0 - 0.25) * 0.5;
            particles.list.push(s);
            p += step;
        }

        start += left;
        end += right;
    }
}

pub(super) fn break_pane(particles: &mut Particles, level: &mut Level, cell: IVec3, face: i32, hit: Vec3, hit_vel: Vec3) -> bool {
    let area = &level.map.level.area;
    let word = area.layer1(cell.x, cell.y, cell.z);

    if word & TYPE_MASK != MESH {
        return false;
    }

    let bs = area.block_size;
    let wall = level.map.level.meshes.get(word & 0xffff).and_then(|m| m.walls.get(face as usize));

    if let Some(w) = wall {
        let at = |c: Vec3| (c + cell.as_vec3()) * bs + area.origin;
        let c = [at(w[3]), at(w[2]), at(w[1]), at(w[0])];

        for k in 0..4 {
            let tip = if k == 2 { hit - Vec3::Y / 16.0 } else { hit };
            let mut t = Particle::new(Kind::Triangle, Vec3::ZERO, Vec3::ZERO);

            t.frame = [c[k], c[(k + 1) & 3], tip];
            t.corner_vel = [Vec3::ZERO, Vec3::ZERO, hit_vel / 64.0];
            t.life = 3600;
            t.rest = [t.frame[0].distance(t.frame[1]), t.frame[1].distance(t.frame[2]), t.frame[2].distance(t.frame[0])];
            particles.list.push(t);
        }
    }

    let broken = word | 65536u32.checked_shl(face as u32).unwrap_or(0);
    level.map.level.area.set_layer1(cell.x, cell.y, cell.z, broken);

    true
}

pub(super) fn settle(p: &mut Particle, map: &Map) {
    if p.life <= 3600 - 600 {
        return;
    }

    for k in 0..3 {
        p.corner_vel[k].y -= p.gravity;

        let old = p.frame[k];
        p.frame[k] += p.corner_vel[k];

        if let Some((at, n)) = trace(map, old, p.frame[k]) {
            p.frame[k] = at + n * RESTING_LIFT;
            p.corner_vel[k] = bounce(p.corner_vel[k], n, 0.875);
        }
    }

    let snap = p.corner_vel;

    for j in 0..3 {
        let next = (j + 1) % 3;
        let d = p.frame[next] - p.frame[j];
        let dir = d.normalize_or_zero();
        let push = dir.dot(snap[next]) * 0.25 + ((d.length() - p.rest[j]) * 0.125 - dir.dot(snap[j]) * 0.25);

        p.corner_vel[j] += dir * push;
        p.corner_vel[next] -= dir * push;
    }
}
