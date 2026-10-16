use std::{
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures::check_ready},
};
use ferrosa_world::{item_types::item_types, map::Map, vehicle_types::vehicle_types};

use super::{Level, blocks, items::ItemTypes, terrain, vehicles::VehicleTypes};
use crate::menu::{Screen, join::ServerSync};

const DEFAULT_MAP: &str = "round";
const DEFAULT_STREETS: (i32, i32, i32, i32) = (1024, 1024, 2600, 2600);

#[derive(Resource)]
pub struct LevelGeometry {
    pub blocks: blocks::Built,
    pub terrain: Vec<terrain::Tile>,
    pub images: Vec<Option<Vec<u8>>>,
}

struct Loaded {
    map: Map,
    geometry: LevelGeometry,
    vehicle_types: VehicleTypes,
    item_types: ItemTypes,
}

#[derive(Resource)]
struct Loading(Task<Result<Loaded, String>>);

#[derive(Component)]
struct LoadingText;

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::Loading), start_loading)
            .add_systems(Update, finish_loading.run_if(in_state(Screen::Loading)));
    }
}

fn map_dir(name: &str) -> PathBuf {
    Path::new("data").join(name)
}

fn read_images(map: &Map) -> Vec<Option<Vec<u8>>> {
    let dir = map_dir(&map.map_name);
    let mut images = vec![None, std::fs::read(dir.join("asphalt2.png")).ok(), None, None];
    let mut csx = std::fs::File::open(dir.join("test.csx")).ok();

    for &(offset, size, _) in &map.level.texture_data {
        let image = csx.as_mut().filter(|_| size > 0).and_then(|f| {
            f.seek(SeekFrom::Start(offset)).ok()?;
            let mut bytes = vec![0; size as usize];
            f.read_exact(&mut bytes).ok()?;
            Some(bytes)
        });

        images.push(image);
    }

    images
}

fn street_bounds(map: &Map) -> (i32, i32, i32, i32) {
    let points: Vec<Vec3> = map.streets.intersections.iter().map(|i| i.world_pos).collect();

    if points.is_empty() {
        return DEFAULT_STREETS;
    }

    let lo = points.iter().fold(Vec3::splat(f32::MAX), |a, p| a.min(*p));
    let hi = points.iter().fold(Vec3::splat(f32::MIN), |a, p| a.max(*p));

    (lo.x as i32, lo.z as i32, hi.x as i32, hi.z as i32)
}

fn load(name: String) -> Result<Loaded, String> {
    let map = Map::load_map(name.clone()).map_err(|e| format!("{name}: {e}"))?;

    let geometry = LevelGeometry {
        blocks: blocks::build(&map),
        terrain: terrain::build(&map.ground, street_bounds(&map)),
        images: read_images(&map),
    };

    Ok(Loaded { map, geometry, vehicle_types: VehicleTypes(vehicle_types(Path::new("data"))), item_types: ItemTypes(item_types()) })
}

fn start_loading(mut commands: Commands, sync: Option<Res<ServerSync>>) {
    let name = sync.map_or_else(|| DEFAULT_MAP.to_string(), |s| s.map.clone());

    commands.spawn((
        Node { width: percent(100), height: percent(100), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
        DespawnOnExit(Screen::Loading),
        children![(Text::new(format!("Loading {name}...")), LoadingText)],
    ));

    commands.insert_resource(Loading(AsyncComputeTaskPool::get().spawn(async move { load(name) })));
}

fn finish_loading(mut commands: Commands, mut loading: ResMut<Loading>, mut next: ResMut<NextState<Screen>>, mut text: Single<&mut Text, With<LoadingText>>) {
    let Some(result) = check_ready(&mut loading.0) else { return };

    commands.remove_resource::<Loading>();

    match result {
        Ok(loaded) => {
            commands.insert_resource(Level { map: loaded.map });
            commands.insert_resource(loaded.geometry);
            commands.insert_resource(loaded.vehicle_types);
            commands.insert_resource(loaded.item_types);
            next.set(Screen::InGame);
        }
        Err(e) => {
            error!("could not load the map: {e}");
            text.0 = format!("Could not load the map: {e}");
        }
    }
}
