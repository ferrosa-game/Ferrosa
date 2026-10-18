use bevy::{gltf::GltfAssetLabel, prelude::*};
use ferrosa_world::{item_sets::entry_pose, item_types::model_name};

use super::{
    Level,
    items::ItemTypes,
    netgame::{CellChange, NetGame},
};
use crate::menu::Screen;

const SET_MASK: u32 = 1023;
const TURN_SHIFT: u32 = 24;
const TURN_MASK: u32 = 3;

#[derive(Component)]
struct SetItem(IVec3);

#[derive(Resource, Default)]
struct DrawnRound(Option<u32>);

pub struct SetItemsPlugin;

impl Plugin for SetItemsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DrawnRound>()
            .add_systems(Update, (redraw_on_new_round, apply_cell_changes).chain().run_if(in_state(Screen::InGame)).run_if(resource_exists::<NetGame>).run_if(resource_exists::<ItemTypes>))
            .add_systems(OnExit(Screen::InGame), |mut drawn: ResMut<DrawnRound>| drawn.0 = None);
    }
}

fn set_cells(level: &Level) -> Vec<IVec3> {
    let area = &level.map.level.area;

    area.blocks()
        .flat_map(|(base, record)| {
            record.item_set.iter().enumerate().filter(|&(_, &word)| word & SET_MASK != 0).map(move |(k, _)| base + IVec3::new((k & 7) as i32, (k >> 6) as i32, ((k >> 3) & 7) as i32))
        })
        .collect()
}

fn draw_cell(commands: &mut Commands, assets: &AssetServer, level: &Level, types: &ItemTypes, cell: IVec3) {
    let area = &level.map.level.area;
    let word = area.item_set(cell.x, cell.y, cell.z);
    let Some(set) = area.item_sets.set(word).filter(|_| word & SET_MASK != 0) else { return };
    let turn = (word >> TURN_SHIFT) & TURN_MASK;
    let taken = area.taken(cell.x, cell.y, cell.z);

    for (i, entry) in set.entries.iter().enumerate() {
        if taken & 1u32.wrapping_shl(i as u32) != 0 {
            continue;
        }

        let Some(kind) = usize::try_from(entry.kind).ok() else { continue };
        let Some(ty) = types.0.get(kind) else { continue };
        let (pos, rot) = entry_pose(entry, cell, area.block_size, turn);
        let place = Transform::from_translation(pos + area.origin).with_rotation(Quat::from_mat3(&Mat3::from_cols(rot[0], rot[1], rot[2])));
        let model = assets.load(GltfAssetLabel::Scene(0).from_asset(format!("models/items/{}.glb", model_name(kind, &ty.name))));

        commands.spawn((SetItem(cell), WorldAssetRoot(model), place, DespawnOnExit(Screen::InGame)));
    }
}

fn redraw_on_new_round(
    mut commands: Commands,
    assets: Res<AssetServer>,
    net: Res<NetGame>,
    mut level: ResMut<Level>,
    types: Res<ItemTypes>,
    mut drawn: ResMut<DrawnRound>,
    items: Query<Entity, With<SetItem>>,
) {
    if drawn.0 == Some(net.round) {
        return;
    }

    items.iter().for_each(|e| commands.entity(e).despawn());

    let cells = set_cells(&level);

    for &cell in &cells {
        level.map.level.area.set_taken(cell.x, cell.y, cell.z, 0);
    }

    for &cell in &cells {
        draw_cell(&mut commands, &assets, &level, &types, cell);
    }

    drawn.0 = Some(net.round);
}

fn apply_cell_changes(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut net: ResMut<NetGame>,
    mut level: ResMut<Level>,
    types: Res<ItemTypes>,
    items: Query<(Entity, &SetItem)>,
) {
    let mut changed: Vec<IVec3> = Vec::new();

    for change in std::mem::take(&mut net.cell_changes) {
        let cell = match change {
            CellChange::Taken { cell, mask } => {
                level.map.level.area.set_taken(cell.x, cell.y, cell.z, mask);
                cell
            }
            CellChange::ItemSet { cell, value } => {
                level.map.level.area.set_item_set(cell.x, cell.y, cell.z, value);
                cell
            }
        };

        if !changed.contains(&cell) {
            changed.push(cell);
        }
    }

    for cell in changed {
        items.iter().filter(|(_, s)| s.0 == cell).for_each(|(e, _)| commands.entity(e).despawn());
        draw_cell(&mut commands, &assets, &level, &types, cell);
    }
}
