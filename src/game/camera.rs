use std::f32::consts::FRAC_PI_2;

use bevy::{
    camera::{ClearColorConfig, Exposure, Hdr}, core_pipeline::tonemapping::Tonemapping, light::AtmosphereEnvironmentMapLight, pbr::AtmosphereSettings, post_process::bloom::Bloom, prelude::*, render::view::NoIndirectDrawing, window::{CursorGrabMode, CursorOptions},
};
use ferrosa_world::{map::Map, skeleton::BoneId, track::Track};

use bevy_enhanced_input::prelude::Action;
use ferrosa_protocol::GameMode;

use super::{
    GAME_FPS, Level,
    controls::{Fly, FlyFast, FlyLook, FlySlow, FlyZoom},
    escape_menu::GameMenu,
    first_person::FirstPerson,
    human_sync::{NetHuman, bone_at},
    netgame::NetGame,
    smoothing::Smoothing,
    spectating::spectates,
};
use crate::{config::Config, character_model::HumanPose, menu::Screen};

#[derive(Component)]
pub struct GameCamera;

#[derive(Component, Default)]
pub struct Spectator {
    pos: Vec3,
    yaw: f32,
    pitch: f32,
    zoom: f32,
    zoom_target: f32,
}

impl Spectator {
    fn transform(&self) -> Transform {
        let rotation = Quat::from_rotation_y(-self.yaw) * Quat::from_rotation_x(-self.pitch);

        Transform { translation: self.pos + rotation * Vec3::Z * self.zoom, rotation, ..default() }
    }
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::InGame), (spawn_camera, draw_menus_over_game))
            .add_systems(OnExit(Screen::InGame), restore_menu_camera)
            .add_systems(Update, capture_cursor)
            .add_systems(Update, change_look.run_if(resource_changed::<Config>).run_if(in_state(Screen::InGame)))
            .add_systems(Update, fly_spectator.in_set(Smoothing::View).run_if(in_state(GameMenu::Closed)).run_if(|fp: Res<FirstPerson>| !fp.active));
    }
}

fn spectator_start(map: &Map) -> Spectator {
    let street = map
        .streets
        .streets
        .first()
        .and_then(|s| map.streets.intersections.get(s.intersections[0]))
        .map_or(Vec3::ZERO, |i| i.world_pos + Vec3::new(1480.0, 1.0, -24.0));

    let spawn = match map.map_name.as_str() {
        "test2" => Track::race_track().spawns.get(5).map_or(street, |&(p, _)| p + Vec3::new(16.0, 1.0, 0.0)),
        _ => street,
    };

    Spectator { pos: spawn + Vec3::new(32.0, 16.0, 0.0), yaw: -FRAC_PI_2, ..default() }
}

pub fn exposure(config: &Config) -> Exposure {
    if config.rosa_colours != 0 { Exposure { ev100: 13.5 } } else { Exposure { ev100: 14.5 } }
}

pub fn tonemapping(config: &Config) -> Tonemapping {
    if config.rosa_colours != 0 { Tonemapping::Reinhard } else { Tonemapping::AcesFitted }
}

fn look(camera: &mut EntityCommands, config: &Config) {
    camera.insert(tonemapping(config));
    camera.insert(exposure(config));

    if config.rosa_colours != 0 {
        camera.remove::<Bloom>();
    } else {
        camera.insert(Bloom::OLD_SCHOOL);
    }
}

fn change_look(mut commands: Commands, config: Res<Config>, cameras: Query<Entity, With<GameCamera>>) {
    for e in &cameras {
        look(&mut commands.entity(e), &config);
    }
}

fn spawn_camera(mut commands: Commands, level: Res<Level>, config: Res<Config>) {
    let spectator = spectator_start(&level.map);

    let mut camera = commands.spawn((
        GameCamera,
        Camera3d::default(),
        Camera { order: -1, ..default() },
        NoIndirectDrawing, // This fixes a flashing issue on Dx12, why it exists? Idk.
        AtmosphereSettings::default(),
        AtmosphereEnvironmentMapLight::default(),
        spectator.transform(),
        spectator,
        DespawnOnExit(Screen::InGame),
    ));

    look(&mut camera, &config);
}

fn draw_menus_over_game(mut commands: Commands, mut cameras: Query<(Entity, &mut Camera), With<Camera2d>>) {
    for (e, mut camera) in &mut cameras {
        camera.clear_color = ClearColorConfig::None;
        commands.entity(e).insert(Hdr);
    }
}

fn restore_menu_camera(mut commands: Commands, mut cameras: Query<(Entity, &mut Camera), With<Camera2d>>) {
    for (e, mut camera) in &mut cameras {
        camera.clear_color = ClearColorConfig::Default;
        commands.entity(e).remove::<Hdr>();
    }
}

fn capture_cursor(menu: Option<Res<State<GameMenu>>>, window: Single<(&mut Window, &mut CursorOptions)>) {
    let (mut window, mut cursor) = window.into_inner();
    let playing = menu.is_some_and(|m| matches!(m.get(), GameMenu::Closed | GameMenu::Chat));
    let grab_mode = if playing { CursorGrabMode::Confined } else { CursorGrabMode::None };

    if cursor.grab_mode == grab_mode && cursor.visible == !playing {
        return;
    }

    if playing {
        let middle = window.size() / 2.0;
        window.set_cursor_position(Some(middle));
    }

    cursor.grab_mode = grab_mode;
    cursor.visible = !playing;
}

fn fly_spectator(
    time: Res<Time>,
    config: Res<Config>,
    (fly, look, zoom, fast, slow): (Single<&Action<Fly>>, Single<&Action<FlyLook>>, Single<&Action<FlyZoom>>, Single<&Action<FlyFast>>, Single<&Action<FlySlow>>),
    net: Option<Res<NetGame>>,
    humans: Query<(&NetHuman, &Transform, &HumanPose), Without<Spectator>>,
    camera: Single<(&mut Transform, &mut Spectator)>,
) {
    let (mut transform, mut spectator) = camera.into_inner();
    let frames = time.delta_secs() * GAME_FPS;
    let target = net.filter(|n| spectates(n.sync.game_type) || n.sync.game_type == GameMode::World).and_then(|n| usize::try_from(n.spectating).ok());
    let watched = target.and_then(|t| humans.iter().find(|(nh, ..)| nh.0 == t)).map(|(_, at, pose)| bone_at(at.translation, pose, BoneId::Head.index()).0);

    if let Some(head) = watched {
        spectator.pos = head;
    }

    let speed = 0.25 * if ***fast { 4.0 } else { 1.0 } * if ***slow { 0.25 } else { 1.0 };
    let (across, up, back) = (fly.x, fly.y, fly.z);
    let (sin, cos) = spectator.yaw.sin_cos();

    if watched.is_none() {
        spectator.pos += Vec3::new(-back * sin + across * cos, up, across * sin + back * cos) * speed * frames;
    }

    let turn = (1.0 / 320.0) * config.sensitivity();

    spectator.yaw += look.x * turn;
    spectator.pitch = (spectator.pitch + look.y * turn).clamp(-FRAC_PI_2, FRAC_PI_2);

    spectator.zoom_target = (spectator.zoom_target - zoom.y).clamp(2.0, 32.0);
    spectator.zoom += (spectator.zoom_target - spectator.zoom) * (1.0 - (1.0 - 0.0625_f32).powf(frames));

    *transform = spectator.transform();
}
