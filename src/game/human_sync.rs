use bevy::prelude::*;
use ferrosa_protocol::CharacterCustomization;
use ferrosa_world::skeleton::BONES as TEMPLATES;

use super::{
    netgame::NetGame,
    smoothing::{Smoothing, Snapshots},
};
use crate::{
    character_model::{BONES, HumanLook, HumanPose, PARENTS},
    menu::Screen,
};

#[derive(Component)]
pub struct NetHuman(pub usize);

pub struct HumansPlugin;

impl Plugin for HumansPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, sync_humans.in_set(Smoothing::Sync).run_if(resource_exists::<NetGame>));
    }
}

fn look(c: &CharacterCustomization) -> HumanLook {
    HumanLook {
        gender: c.gender as u32,
        head: c.head as u32,
        hair: c.hair_style as u32,
        skin_colour: c.skin as u32,
        hair_colour: c.hair_color as u32,
        eye_colour: c.eye_color as u32,
        model: c.model as u32,
        suit_colour: c.suit_color as u32,
        tie_colour: c.tie_color as u32,
        necklace: c.necklace as u32,
    }
}

pub fn bone_at(pos: Vec3, pose: &HumanPose, bone: usize) -> (Vec3, Quat) {
    if bone == 0 {
        return (pos, pose.0[0]);
    }

    let (parent, template) = (PARENTS[bone], &TEMPLATES[bone]);
    let (at, _) = bone_at(pos, pose, parent);

    (at + pose.0[parent] * template.joint + pose.0[bone] * (template.offset - template.joint), pose.0[bone])
}

pub fn pose(root: Quat, bones: &[Quat; BONES - 1]) -> HumanPose {
    let mut world = [root.conjugate(); BONES];

    for j in 1..BONES {
        world[j] = world[PARENTS[j]] * bones[j - 1].conjugate();
    }

    HumanPose(world)
}

fn sync_humans(mut commands: Commands, net: Res<NetGame>, mut humans: Query<(Entity, &NetHuman, &mut HumanLook, &mut Snapshots<Transform>, &mut Snapshots<HumanPose>)>) {
    let mut drawn = vec![false; net.humans.len()];

    for (e, &NetHuman(i), mut human_look, mut places, mut poses) in &mut humans {
        let Some(h) = net.humans.get(i).filter(|h| h.active) else {
            commands.entity(e).despawn();
            continue;
        };

        drawn[i] = true;
        human_look.set_if_neq(look(&h.look));

        if places.last_tick() != Some(h.track.tick) {
            places.push(h.track.tick, Transform::from_translation(h.track.pos));
            poses.push(h.track.tick, pose(h.rot, &h.bones));
        }
    }

    for (i, h) in net.humans.iter().enumerate().filter(|(i, h)| h.active && !drawn[*i]) {
        let (place, posed) = (Transform::from_translation(h.track.pos), pose(h.rot, &h.bones));
        let gliding = (Snapshots::new(h.track.tick, place), Snapshots::new(h.track.tick, posed));

        commands.spawn((NetHuman(i), look(&h.look), posed, place, gliding, DespawnOnExit(Screen::InGame)));
    }
}
