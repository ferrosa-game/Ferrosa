use bevy::{gltf::GltfAssetLabel, prelude::*};
use ferrosa_world::city_objects::{GARAGE_DOOR, object_pose};

use super::{Level, netgame::NetGame};
use crate::menu::Screen;

const DOOR_MODEL: &str = "models/objects/object_6.glb";

#[derive(Component)]
struct TeamDoor(usize);

pub struct DoorsPlugin;

impl Plugin for DoorsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::InGame), spawn_doors).add_systems(Update, shut_doors.run_if(in_state(Screen::InGame)).run_if(resource_exists::<NetGame>));
    }
}

fn spawn_doors(mut commands: Commands, assets: Res<AssetServer>, level: Res<Level>) {
    let area = &level.map.level.area;
    let model = assets.load(GltfAssetLabel::Scene(0).from_asset(DOOR_MODEL));

    for (team, base) in level.map.level.bases.iter().enumerate() {
        let Some((cells, dir)) = base.door else { continue };

        for cell in cells {
            let (_, pos, rot) = object_pose(dir << 10 | GARAGE_DOOR, cell, area.block_size);
            let place = Transform::from_translation(pos + area.origin).with_rotation(Quat::from_mat3(&Mat3::from_cols(rot[0], rot[1], rot[2])));

            commands.spawn((TeamDoor(team), WorldAssetRoot(model.clone()), place, DespawnOnExit(Screen::InGame)));
        }
    }
}

fn shut_doors(net: Res<NetGame>, mut doors: Query<(&TeamDoor, &mut Visibility)>) {
    for (&TeamDoor(team), mut visibility) in &mut doors {
        let open = net.team_doors.get(team).copied().unwrap_or(false);

        visibility.set_if_neq(if open { Visibility::Hidden } else { Visibility::Inherited });
    }
}
