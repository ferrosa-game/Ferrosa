use bevy::prelude::*;
use ferrosa_world::item_types::ItemType;

use super::netgame::NetGame;
use crate::menu::Screen;

const ITEM_MARK: Color = Color::srgb(1.0, 0.85, 0.2);
const ITEM_SIZE: f32 = 0.3;

#[derive(Resource)]
pub struct ItemTypes(pub Vec<ItemType>);

pub struct ItemsPlugin;

impl Plugin for ItemsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, draw_loose_items.run_if(in_state(Screen::InGame)).run_if(resource_exists::<NetGame>));
    }
}

// TODO: the item models, and items held and carried.
fn draw_loose_items(net: Res<NetGame>, mut gizmos: Gizmos) {
    for item in net.items.iter().filter(|i| i.active && i.parent_human < 0 && i.parent_item < 0) {
        gizmos.cube(Transform { translation: item.track.pos, rotation: item.rot.conjugate(), scale: Vec3::splat(ITEM_SIZE) }, ITEM_MARK);
    }
}
