use bevy::{
    camera::visibility::RenderLayers,
    light::{Atmosphere, CascadeShadowConfigBuilder, DirectionalLightShadowMap, GlobalAmbientLight, SunDisk, atmosphere::ScatteringMedium, light_consts::lux},
    prelude::*,
};

use crate::{
    character_model::SHADOW_ONLY,
    game::netgame::{NetGame, sync::ServerSync},
    menu::Screen,
};

const DEFAULT_SUN_ANGLE: f32 = 0.8;
const WORLD_HOUR: f32 = 216_000.0;
const WORLD_DAY: f32 = 24.0 * WORLD_HOUR;
const CLOCK_BASE: f32 = 11.0 * WORLD_HOUR;
const FILL: Color = Color::srgb(1.0, 0.85, 0.65);
const FILL_BRIGHTNESS: f32 = 6000.0;
const SHADOW_NEAR: f32 = 30.0;
const SHADOW_FAR: f32 = 3000.0;
const SHADOW_MAP: usize = 4096;

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::InGame), spawn_sky)
            .add_systems(Update, turn_sun.run_if(in_state(Screen::InGame)).run_if(resource_exists::<NetGame>))
            .add_systems(OnExit(Screen::InGame), |mut commands: Commands| commands.insert_resource(GlobalAmbientLight::default()));
    }
}

#[derive(Component)]
struct Sun(f32);

fn turn_sun(net: Res<NetGame>, mut suns: Query<(&Sun, &mut Transform)>) {
    let turn = (net.sun_time as f32 - CLOCK_BASE) / WORLD_DAY * std::f32::consts::TAU;

    for (sun, mut place) in &mut suns {
        *place = Transform::default().looking_to(-sun_direction(sun.0 + turn), Vec3::Y);
    }
}

fn sun_direction(angle: f32) -> Vec3 {
    let (a, b) = (f32::from_bits(0x3f64f92e), f32::from_bits(0x3ee4f92e));

    Quat::from_axis_angle(Vec3::new(0.0, b, -a).normalize(), angle) * Vec3::new(0.0, a, b)
}

fn spawn_sky(mut commands: Commands, sync: Option<Res<ServerSync>>, mut media: ResMut<Assets<ScatteringMedium>>) {
    let angle = sync.map_or(DEFAULT_SUN_ANGLE, |s| s.sun_angle);

    commands.spawn((
        DirectionalLight { illuminance: lux::RAW_SUNLIGHT, shadow_maps_enabled: true, ..default() },
        RenderLayers::from_layers(&[0, SHADOW_ONLY]),
        SunDisk::EARTH,
        CascadeShadowConfigBuilder { first_cascade_far_bound: SHADOW_NEAR, maximum_distance: SHADOW_FAR, ..default() }.build(),
        Transform::default().looking_to(-sun_direction(angle), Vec3::Y),
        Sun(angle),
        DespawnOnExit(Screen::InGame),
    ));

    commands.spawn((Atmosphere::earth(media.add(ScatteringMedium::earth(256, 256))), DespawnOnExit(Screen::InGame)));

    commands.insert_resource(DirectionalLightShadowMap { size: SHADOW_MAP });
    commands.insert_resource(GlobalAmbientLight { color: FILL, brightness: FILL_BRIGHTNESS, ..default() });
}
