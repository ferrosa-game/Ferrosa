use bevy::{
    camera::{Exposure, RenderTarget, ScalingMode},
    core_pipeline::tonemapping::Tonemapping,
    prelude::*,
    render::render_resource::TextureFormat,
};

use super::{MAP_GROUND, MapPicture, SPAN};
use crate::{
    game::{Level, camera::EXPOSURE},
    menu::Screen,
};

const ROUND_ORIGIN: Vec2 = Vec2::new(960.0, 896.0);
const CITY_ORIGIN: Vec2 = Vec2::new(896.0, 896.0);
const ROUND_SCALE: f32 = 0.25;
const CITY_SCALE: f32 = 0.125;
const PICTURE_SIZE: u32 = 1024;
const PICTURE_FRAMES: u32 = 8;
const CAMERA_HEIGHT: f32 = 1000.0;

#[derive(Component)]
pub(super) struct PictureCamera(u32);

pub(super) fn prepare_picture(mut commands: Commands, assets: Res<AssetServer>, level: Res<Level>, mut images: ResMut<Assets<Image>>) {
    if level.map.map_name == "round" {
        commands.insert_resource(MapPicture { image: assets.load("round/map.png"), origin: ROUND_ORIGIN, scale: ROUND_SCALE, round: true });
        return;
    }

    let image = images.add(Image::new_target_texture(PICTURE_SIZE, PICTURE_SIZE, TextureFormat::Rgba8UnormSrgb, None));
    let span = SPAN / CITY_SCALE;
    let centre = CITY_ORIGIN + Vec2::splat(span * 0.5);
    let projection = OrthographicProjection { scaling_mode: ScalingMode::Fixed { width: span, height: span }, near: 0.0, far: CAMERA_HEIGHT * 2.0, ..OrthographicProjection::default_3d() };

    commands.spawn((
        PictureCamera(0),
        Camera3d::default(),
        Camera { order: -2, clear_color: ClearColorConfig::Custom(MAP_GROUND), ..default() },
        RenderTarget::Image(image.clone().into()),
        Projection::Orthographic(projection),
        Exposure { ev100: EXPOSURE },
        Tonemapping::AcesFitted,
        Transform::from_xyz(centre.x, CAMERA_HEIGHT, centre.y).looking_at(Vec3::new(centre.x, 0.0, centre.y), Vec3::NEG_Z),
        DespawnOnExit(Screen::InGame),
    ));

    commands.insert_resource(MapPicture { image, origin: CITY_ORIGIN, scale: CITY_SCALE, round: false });
}

pub(super) fn finish_picture(mut commands: Commands, mut cameras: Query<(Entity, &mut PictureCamera)>) {
    for (e, mut camera) in &mut cameras {
        camera.0 += 1;

        if camera.0 > PICTURE_FRAMES {
            commands.entity(e).despawn();
        }
    }
}
