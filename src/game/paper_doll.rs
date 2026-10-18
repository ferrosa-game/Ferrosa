use bevy::prelude::*;
use ferrosa_protocol::clientbound::game::{GameState, ItemKind};
use ferrosa_world::skeleton::BONES;

use super::{
    hud::{self, Stick, HudScreen, in_hand, is_phone, line, own, spawn_screen},
    human_sync::NetHuman,
    netgame::NetGame,
};
use crate::{
    character_model::{self, HumanPose, PARENTS},
    menu::Screen,
};

const AT: Vec2 = Vec2::new(512.0, 444.0);
const SCALE: f32 = 64.0;
const PARTS: [usize; character_model::BONES] = [0, 0, 0, 1, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5, 5, 5];
const FULL_HEALTH: i32 = 100;
const HALF_HEALTH: i32 = 50;
const BLEEDING_ABOVE: i32 = 87;

#[derive(Component)]
struct DollScreen;

#[derive(Component)]
struct DollBone(usize);

pub struct PaperDollPlugin;

impl Plugin for PaperDollPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::InGame), spawn_doll)
            .add_systems(Update, show_doll.run_if(in_state(Screen::InGame)).run_if(resource_exists::<NetGame>).after(super::smoothing::Smoothing::Glide));
    }
}

fn spawn_doll(mut commands: Commands) {
    let screen = spawn_screen(&mut commands, (HudScreen, DollScreen), Stick::Bottom);
    let doll = commands.spawn((Node { position_type: PositionType::Absolute, ..default() }, Pickable::IGNORE, ChildOf(screen))).id();

    for j in 0..character_model::BONES {
        let node = Node { border: UiRect::all(px(1)), ..default() };
        commands.spawn((node, UiTransform::default(), BackgroundColor(Color::NONE), BorderColor::all(Color::NONE), DollBone(j), Pickable::IGNORE, ChildOf(doll)));
    }
}

fn doll_bones(pose: &HumanPose, view_yaw: f32) -> [(Vec3, Quat); character_model::BONES] {
    let facing = Quat::from_rotation_y(-view_yaw).inverse();
    let rots = pose.0.map(|r| facing * r);
    let mut pos = [Vec3::ZERO; character_model::BONES];

    for j in 1..character_model::BONES {
        let (b, p) = (&BONES[j], PARENTS[j]);
        pos[j] = pos[p] + rots[p] * b.joint + rots[j] * (b.offset - b.joint);
    }

    std::array::from_fn(|j| (pos[j], rots[j]))
}

fn health_colour(health: i32) -> (f32, f32) {
    if health >= HALF_HEALTH { (1.0, (health - HALF_HEALTH) as f32 / HALF_HEALTH as f32) } else { (health as f32 / HALF_HEALTH as f32, 0.0) }
}

fn show_doll(
    net: Res<NetGame>,
    humans: Query<(&NetHuman, &HumanPose)>,
    mut screens: Query<&mut Visibility, (With<DollScreen>, Without<DollBone>)>,
    mut bones: Query<(&DollBone, &mut Node, &mut UiTransform, &mut BackgroundColor, &mut BorderColor)>,
) {
    let ours = own(&net).filter(|(_, h, _)| {
        let held = in_hand(&net, h, 0).map(|(_, kind)| kind);
        net.game_state != GameState::Restarting && !held.is_some_and(|k| is_phone(k) || k == ItemKind::Computer)
    });
    let posed = ours.and_then(|(p, h, own)| humans.iter().find(|(nh, _)| nh.0 as i32 == p.human).map(|(_, pose)| (h, own, doll_bones(pose, own.view_yaw))));

    for mut v in &mut screens {
        v.set_if_neq(if posed.is_some() { Visibility::Inherited } else { Visibility::Hidden });
    }

    let Some((h, own, placed)) = posed else { return };

    for (&DollBone(j), mut node, mut turn, mut fill, mut edge) in &mut bones {
        let b = &BONES[j];
        let (pos, rot) = placed[j];
        let axis = rot * Vec3::AXES[b.shape.clamp(0, 2) as usize];
        let half = b.shape_size[0] * 0.5;
        let screen = |p: Vec3| Vec2::new(AT.x + p.x * SCALE, AT.y - p.y * SCALE);
        let radius = SCALE * b.shape_size[1];

        (*node, *turn) = {
            let (n, t) = line(screen(pos - axis * half), screen(pos + axis * half), radius * 2.0, true);
            (Node { border: UiRect::all(px(1)), ..n }, t)
        };

        let health = own.health[PARTS[j]];
        let (green, blue) = health_colour(health);
        let filled = if h.bleeding && health > BLEEDING_ABOVE {
            Some([1.0, 0.0, 0.0, (FULL_HEALTH - BLEEDING_ABOVE) as f32 / HALF_HEALTH as f32])
        } else {
            (health < FULL_HEALTH).then(|| [1.0, green, blue, if health < HALF_HEALTH { 1.0 } else { (FULL_HEALTH - health) as f32 / HALF_HEALTH as f32 }])
        };

        fill.0 = filled.map_or(Color::NONE, hud::colour);
        *edge = BorderColor::all(hud::colour([1.0, green, blue, 0.5]));
    }
}
