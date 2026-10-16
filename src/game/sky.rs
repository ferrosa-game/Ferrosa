use bevy::{
    light::{Atmosphere, CascadeShadowConfigBuilder, DirectionalLightShadowMap, GlobalAmbientLight, SunDisk, atmosphere::ScatteringMedium, light_consts::lux},
    prelude::*,
};

use crate::menu::{Screen, join::ServerSync};

const DEFAULT_SUN_ANGLE: f32 = 0.8;
const FILL: Color = Color::srgb(1.0, 0.85, 0.65);
const FILL_BRIGHTNESS: f32 = 6000.0;
const SHADOW_NEAR: f32 = 30.0;
const SHADOW_FAR: f32 = 3000.0;
const SHADOW_MAP: usize = 4096;

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::InGame), spawn_sky)
            .add_systems(OnExit(Screen::InGame), |mut commands: Commands| commands.insert_resource(GlobalAmbientLight::default()));
    }
}

fn sun_direction(angle: f32) -> Vec3 {
    let (a, b) = (f32::from_bits(0x3f64f92e), f32::from_bits(0x3ee4f92e));

    Quat::from_axis_angle(Vec3::new(0.0, b, -a).normalize(), angle) * Vec3::new(0.0, a, b)
}

fn spawn_sky(mut commands: Commands, sync: Option<Res<ServerSync>>, mut media: ResMut<Assets<ScatteringMedium>>) {
    let sun = sun_direction(sync.map_or(DEFAULT_SUN_ANGLE, |s| s.sun_angle));

    commands.spawn((
        DirectionalLight { illuminance: lux::RAW_SUNLIGHT, shadow_maps_enabled: true, ..default() },
        SunDisk::EARTH,
        CascadeShadowConfigBuilder { first_cascade_far_bound: SHADOW_NEAR, maximum_distance: SHADOW_FAR, ..default() }.build(),
        Transform::default().looking_to(-sun, Vec3::Y),
        DespawnOnExit(Screen::InGame),
    ));

    commands.spawn((Atmosphere::earth(media.add(ScatteringMedium::earth(256, 256))), DespawnOnExit(Screen::InGame)));

    commands.insert_resource(DirectionalLightShadowMap { size: SHADOW_MAP });
    commands.insert_resource(GlobalAmbientLight { color: FILL, brightness: FILL_BRIGHTNESS, ..default() });
}
