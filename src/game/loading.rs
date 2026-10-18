use std::{
    io::{Read, Seek, SeekFrom},
    path::Path,
};

use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures::check_ready},
    ui_widgets::Activate,
};
use ferrosa_world::{item_types::item_types, map::Map, vehicle_types::vehicle_types};

use super::{Level, blocks, items::ItemTypes, netgame::NetGame, terrain, vehicles::VehicleTypes};
use crate::{
    menu::{Screen, join::ServerSync},
    net::ClientSocket,
    ui::{self, Shortcut},
};

const DEFAULT_MAP: &str = "round";
const DEFAULT_STREETS: (i32, i32, i32, i32) = (1024, 1024, 2600, 2600);
const BACK: Vec2 = Vec2::new(320.0, 44.0);

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
struct Loading {
    map: String,
    task: Task<Result<Loaded, String>>,
}

#[derive(Component)]
struct LoadingScreen;

#[derive(Component)]
struct LoadingText;

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::Loading), start_loading)
            .add_systems(Update, finish_loading.run_if(in_state(Screen::Loading)));
    }
}

fn read_images(map: &Map) -> Vec<Option<Vec<u8>>> {
    let dir = Path::new("data").join(map.map_name.clone());
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
        Node {
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: px(24),
            ..default()
        },
        LoadingScreen,
        DespawnOnExit(Screen::Loading),
        children![(Text::new(format!("Loading {name}...")), LoadingText)],
    ));

    commands.insert_resource(Loading { map: name.clone(), task: AsyncComputeTaskPool::get().spawn(async move { load(name) }) });
}

fn finish_loading(
    mut commands: Commands,
    mut loading: ResMut<Loading>,
    mut next: ResMut<NextState<Screen>>,
    mut text: Single<&mut Text, With<LoadingText>>,
    screen: Single<Entity, With<LoadingScreen>>,
) {
    let Some(result) = check_ready(&mut loading.task) else { return };

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
            text.0 = format!("Could not find the map {}!", loading.map);

            let back = ui::button(&mut commands, *screen, "Back", BACK);
            commands.entity(back).insert(Shortcut(KeyCode::Escape)).observe(leave);
        }
    }
}

fn leave(_: On<Activate>, mut commands: Commands, net: Option<Res<NetGame>>, mut socket: ResMut<ClientSocket>, mut next: ResMut<NextState<Screen>>) {
    if let Some(net) = net {
        net.leave(&mut socket);
        commands.remove_resource::<NetGame>();
    }

    next.set(Screen::Main);
}
