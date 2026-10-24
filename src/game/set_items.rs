use bevy::{gltf::GltfAssetLabel, prelude::*};
use ferrosa_world::{item_sets::entry_pose, item_types::model_name};

use ferrosa_protocol::clientbound::game::ItemKind;

use super::{
    Level,
    item_displays::ScreenKind,
    items::ItemTypes,
    netgame::{CellChange, NetGame},
};
use crate::menu::Screen;

#[derive(Component)]
struct SetItem(IVec3, usize);

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
            record.item_set.iter().enumerate().filter(|&(_, &word)| word & 1023 != 0).map(move |(k, _)| base + IVec3::new((k & 7) as i32, (k >> 6) as i32, ((k >> 3) & 7) as i32))
        })
        .collect()
}

fn draw_cell(commands: &mut Commands, assets: &AssetServer, level: &Level, types: &ItemTypes, cell: IVec3, kept: &[usize]) {
    let area = &level.map.level.area;
    let word = area.item_set(cell.x, cell.y, cell.z);
    let Some(set) = area.item_sets.set(word).filter(|_| word & 1023 != 0) else { return };
    let turn = (word >> 24) & 3;
    let taken = area.taken(cell.x, cell.y, cell.z);

    for (i, entry) in set.entries.iter().enumerate() {
        if taken & 1u32.wrapping_shl(i as u32) != 0 || kept.contains(&i) {
            continue;
        }

        let Some(kind) = usize::try_from(entry.kind).ok() else { continue };
        let Some(ty) = types.0.get(kind) else { continue };
        let (pos, rot) = entry_pose(entry, cell, area.block_size, turn);
        let place = Transform::from_translation(pos + area.origin).with_rotation(Quat::from_mat3(&Mat3::from_cols(rot[0], rot[1], rot[2])));
        let model = assets.load(GltfAssetLabel::Scene(0).from_asset(format!("models/items/{}.glb", model_name(kind, &ty.name))));

        let mut entity = commands.spawn((SetItem(cell, i), WorldAssetRoot(model), place, DespawnOnExit(Screen::InGame)));

        if let Some(kind) = u8::try_from(kind).ok().and_then(|k| ItemKind::try_from(k).ok()) {
            entity.insert(ScreenKind(kind));
        }
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
        draw_cell(&mut commands, &assets, &level, &types, cell, &[]);
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
    let mut changed: Vec<(IVec3, bool)> = Vec::new();

    for change in std::mem::take(&mut net.cell_changes) {
        let (cell, replaced) = match change {
            CellChange::Taken { cell, mask } => {
                level.map.level.area.set_taken(cell.x, cell.y, cell.z, mask);
                (cell, false)
            }
            CellChange::ItemSet { cell, value } => {
                level.map.level.area.set_item_set(cell.x, cell.y, cell.z, value);
                (cell, true)
            }
        };

        match changed.iter_mut().find(|(c, _)| *c == cell) {
            Some((_, was)) => *was |= replaced,
            None => changed.push((cell, replaced)),
        }
    }

    for (cell, replaced) in changed {
        let taken = level.map.level.area.taken(cell.x, cell.y, cell.z);
        let mut kept: Vec<usize> = Vec::new();

        for (e, item) in items.iter().filter(|(_, s)| s.0 == cell) {
            if replaced || taken & 1u32.wrapping_shl(item.1 as u32) != 0 {
                commands.entity(e).despawn();
            } else {
                kept.push(item.1);
            }
        }

        draw_cell(&mut commands, &assets, &level, &types, cell, &kept);
    }
}
