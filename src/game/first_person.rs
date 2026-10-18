use std::f32::consts::{PI, TAU};

use bevy::prelude::*;
use ferrosa_world::skeleton::BoneId;

use super::{
    camera::GameCamera,
    human_sync::{NetHuman, bone_at, pose},
    netgame::{entities::Human, NetGame},
    network::NetTick,
    smoothing::Smoothing,
};
use crate::character_model::{HideHead, HumanPose};

const CAMERA_TURN: f32 = 0.125;
const MIN_TURN: f32 = 1.525_878_9e-5;
const DEATH_VIEW: f32 = 3.0;

#[derive(Resource)]
pub struct FirstPerson {
    human: i32,
    pos: Vec3,
    rot: Quat,
    spin: Vec3,
    coming: Vec3,
    death_view: f32,
    pub active: bool,
}

impl Default for FirstPerson {
    fn default() -> Self {
        Self { human: -1, pos: Vec3::ZERO, rot: Quat::IDENTITY, spin: Vec3::ZERO, coming: Vec3::ZERO, death_view: 0.0, active: false }
    }
}

pub struct FirstPersonPlugin;

impl Plugin for FirstPersonPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FirstPerson>()
            .add_systems(FixedUpdate, follow_head.in_set(NetTick::Follow))
            .add_systems(Update, (choose_view, hide_own_head, look_through_eyes, change_view_width).chain().in_set(Smoothing::View));
    }
}

fn head_at(pos: Vec3, pose: &HumanPose) -> (Vec3, Quat) {
    bone_at(pos, pose, BoneId::Head.index())
}

fn head(h: &Human) -> (Vec3, Quat) {
    head_at(h.track.pos, &pose(h.rot, &h.bones))
}

fn turn_between(from: Quat, to: Quat) -> Vec3 {
    let d = to * from.inverse();

    if d.w < 0.0 { (-d).to_scaled_axis() } else { d.to_scaled_axis() }
}

fn wrap(a: f32) -> f32 {
    if a < -PI {
        a + TAU
    } else if a > PI {
        a - TAU
    } else {
        a
    }
}

fn angle_difference(a: f32, b: f32) -> f32 {
    let a = if b - a >= PI { a + TAU } else { a };
    let b = if a - b >= PI { b + TAU } else { b };

    a - b
}

fn follow_head(mut fp: ResMut<FirstPerson>, net: Res<NetGame>) {
    let human = net.me().map_or(-1, |p| p.human);
    let (Some(p), Some(h), Some(own)) = (net.me(), usize::try_from(human).ok().and_then(|h| net.humans.get(h)), net.own_human.as_ref()) else {
        fp.human = human;
        return;
    };

    let (head_pos, head_rot) = head(h);

    if fp.human != human {
        (fp.pos, fp.rot, fp.spin, fp.coming) = (head_pos, head_rot, Vec3::ZERO, Vec3::ZERO);
    }

    let last = fp.rot;
    let on_foot = h.vehicle < 0;
    let yaw = wrap(p.view_yaw_delta - angle_difference(own.body_yaw, p.view_yaw));

    let target = if on_foot {
        head_rot * Quat::from_rotation_y(own.yaw_offset - yaw)
    } else {
        let half_yaw = Quat::from_rotation_y((own.yaw_offset - p.view_yaw_delta) * 0.5);
        let half_pitch = Quat::from_rotation_x((own.pitch_offset - p.view_pitch) * 0.5);
        head_rot * half_yaw * half_pitch * half_yaw * half_pitch
    };

    fp.pos = fp.pos.lerp(head_pos, 0.5);
    fp.spin = (turn_between(fp.rot, target) * CAMERA_TURN + fp.spin) * 0.5;

    if fp.spin.length() > MIN_TURN {
        fp.rot = (Quat::from_scaled_axis(fp.spin) * fp.rot).normalize();
    }

    if on_foot && own.is_standing {
        let lean = (fp.rot * Vec3::Y).y;
        let weight = if lean > 0.0 { lean * lean * lean } else { 0.0 };

        let upright = Quat::from_rotation_y(-(yaw + own.view_yaw + 0.5 * p.free_look_yaw))
            * Quat::from_rotation_x(-(0.5 * p.view_pitch + 0.75 * p.free_look_pitch))
            * Quat::from_rotation_y(-0.5 * p.free_look_yaw)
            * Quat::from_rotation_x(-(0.5 * p.view_pitch + 0.25 * p.free_look_pitch));

        let v = turn_between(fp.rot, upright) * (weight * 0.5);
        let len = v.length();

        if len > MIN_TURN {
            fp.rot = (Quat::from_axis_angle(v / len, len.min(weight * std::f32::consts::FRAC_PI_8)) * fp.rot).normalize();
        }
    }

    fp.coming = turn_between(last, fp.rot);
    fp.human = human;
}

fn own_human(net: &NetGame, fp: &FirstPerson) -> Option<usize> {
    fp.active.then(|| net.me().and_then(|p| usize::try_from(p.human).ok())).flatten()
}

fn choose_view(time: Res<Time>, net: Option<Res<NetGame>>, mut fp: ResMut<FirstPerson>) {
    let alive = net.as_ref().and_then(|n| n.me().and_then(|p| usize::try_from(p.human).ok()).map(|h| n.humans[h].alive));

    fp.active = match alive {
        Some(true) => {
            fp.death_view = DEATH_VIEW;
            true
        }
        Some(false) => {
            fp.death_view -= time.delta_secs();
            fp.death_view > 0.0
        }
        None => false,
    };
}

fn hide_own_head(mut commands: Commands, net: Option<Res<NetGame>>, fp: Res<FirstPerson>, humans: Query<(Entity, &NetHuman, Has<HideHead>)>) {
    let own = net.and_then(|n| own_human(&n, &fp));

    for (e, &NetHuman(i), hidden) in &humans {
        match (Some(i) == own, hidden) {
            (true, false) => {
                commands.entity(e).insert(HideHead);
            }
            (false, true) => {
                commands.entity(e).remove::<HideHead>();
            }
            _ => {}
        }
    }
}

fn look_through_eyes(
    net: Option<Res<NetGame>>,
    fp: Res<FirstPerson>,
    time: Res<Time<Fixed>>,
    humans: Query<(&NetHuman, &Transform, &HumanPose), Without<GameCamera>>,
    mut camera: Single<&mut Transform, With<GameCamera>>,
) {
    let Some(net) = net else { return };
    let Some(h) = own_human(&net, &fp) else { return };
    let Some((_, at, posed)) = humans.iter().find(|(nh, ..)| nh.0 == h) else { return };

    let (eyes, _) = head_at(at.translation, posed);

    **camera = Transform::from_translation(eyes).with_rotation(Quat::from_scaled_axis(fp.coming * time.overstep_fraction()) * fp.rot);
}

fn change_view_width(net: Option<Res<NetGame>>, fp: Res<FirstPerson>, mut projection: Single<&mut Projection, With<GameCamera>>) {
    let Projection::Perspective(p) = &mut **projection else { return };
    let zoom = net.as_deref().filter(|n| own_human(n, &fp).is_some()).and_then(|n| n.me()).map_or(0.0, |p| p.zoom_eased);

    p.fov = 2.0 * (0.5625 * (1.0 - zoom)).atan();
}
