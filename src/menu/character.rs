use bevy::{
    camera::RenderTarget,
    input::mouse::MouseScrollUnit,
    picking::events::{PointerDrag, PointerScroll},
    prelude::*,
    render::render_resource::TextureFormat,
    ui_widgets::ValueChange,
};

use crate::{
    config::{Config, config_path},
    human::{HAIRS, HEADS, HumanExtent, HumanLook, HumanPose},
    menu::{Screen, back_button, gap, page, text},
    ui,
};

const SUIT_COLOUR: u32 = 3;
const TURN_PER_PIXEL: f32 = 0.01;
const PITCH_LIMIT: f32 = 1.3;
const NEAREST: f32 = 0.6;
const FARTHEST: f32 = 4.5;
const START_DISTANCE: f32 = 2.9;
const ZOOM_PER_NOTCH: f32 = 0.88;
const PIXELS_PER_NOTCH: f32 = 50.0;
const BODY_HEIGHT: f32 = 0.95;
const FACE_HEIGHT: f32 = 1.72;
pub const PREVIEW: UVec2 = UVec2::new(360, 480);
const PREVIEW_BACKGROUND: Color = Color::srgb(0.08, 0.1, 0.12);

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerLook {
    pub gender: i32,
    pub head: i32,
    pub eyes: i32,
    pub hair: i32,
    pub hair_colour: i32,
    pub skin: i32,
}

impl From<&Config> for PlayerLook {
    fn from(c: &Config) -> Self {
        Self {
            gender: c.player_gender,
            head: c.player_head,
            eyes: c.player_eye_color,
            hair: c.player_hair_style,
            hair_colour: c.player_hair_color,
            skin: c.player_skin_color,
        }
    }
}

pub(super) struct CharacterPreviewPlugin;

impl Plugin for CharacterPreviewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PreviewView>()
            .add_systems(Startup, |mut commands: Commands, mut images: ResMut<Assets<Image>>| {
                commands.insert_resource(PreviewImage(images.add(Image::new_target_texture(PREVIEW.x, PREVIEW.y, TextureFormat::Rgba8UnormSrgb, None))));
            })
            .add_systems(OnEnter(Screen::Character), spawn_scene)
            .add_systems(OnExit(Screen::Character), |mut view: ResMut<PreviewView>| *view = PreviewView::default())
            .add_systems(Update, (wear_player_look, apply_view).run_if(in_state(Screen::Character)));
    }
}

pub(super) fn character(mut commands: Commands, look: Res<PlayerLook>, preview: Res<PreviewImage>) {
    let p = page(&mut commands, Screen::Character);
    text(&mut commands, p, "Character", 32.0, Color::WHITE);
    gap(&mut commands, p, 16.0);

    let row = commands.spawn((Node { column_gap: px(32), align_items: AlignItems::Center, ..default() }, ChildOf(p))).id();

    commands
        .spawn((Node { width: px(PREVIEW.x), height: px(PREVIEW.y), ..default() }, ImageNode::new(preview.0.clone()), ChildOf(row)))
        .observe(drag_preview)
        .observe(zoom_preview);

    let controls = commands.spawn((Node { flex_direction: FlexDirection::Column, row_gap: px(8), ..default() }, ChildOf(row))).id();

    let group = ui::radio::radio_group(&mut commands, controls);
    ui::radio::radio_button(&mut commands, group, "Female", Vec2::new(156.0, 32.0), look.gender == 0);
    let male = ui::radio::radio_button(&mut commands, group, "Male", Vec2::new(156.0, 32.0), look.gender == 1);

    commands.entity(group).observe(move |chosen: On<ValueChange<Entity>>, mut look: ResMut<PlayerLook>|
        look.gender = (chosen.value == male) as i32
    );

    let sliders: [(&str, i32, i32, fn(&mut PlayerLook) -> &mut i32); 5] = [
        ("Head", look.head, HEADS as i32 - 1, |l| &mut l.head),
        ("Eyes", look.eyes, 7, |l| &mut l.eyes),
        ("Hair", look.hair, HAIRS as i32 - 1, |l| &mut l.hair),
        ("Hair Color", look.hair_colour, 0xb, |l| &mut l.hair_colour),
        ("Skin", look.skin, 5, |l| &mut l.skin),
    ];

    for (label, value, max, field) in sliders {
        let s = ui::slider::slider_int(&mut commands, controls, label, value, (0, max), 320.0);

        commands.entity(s).observe(move |v: On<ValueChange<f32>>, mut look: ResMut<PlayerLook>| *field(&mut look) = v.value.round() as i32);
    }

    back_button(&mut commands, p);
}

#[derive(Resource)]
pub struct PreviewImage(Handle<Image>);

#[derive(Component)]
struct PreviewCamera;

#[derive(Component)]
struct PreviewTurntable;

#[derive(Component)]
struct PreviewHuman;

#[derive(Resource, Clone, Copy)]
pub struct PreviewView {
    turn: f32,
    pitch: f32,
    distance: f32,
}

impl Default for PreviewView {
    fn default() -> Self {
        Self { turn: 20f32.to_radians(), pitch: 0.0, distance: START_DISTANCE }
    }
}

fn spawn_scene(mut commands: Commands, preview: Res<PreviewImage>, look: Res<PlayerLook>) {
    commands.spawn((
        Camera3d::default(),
        Camera { clear_color: ClearColorConfig::Custom(PREVIEW_BACKGROUND), order: -1, ..default() },
        RenderTarget::from(preview.0.clone()),
        Transform::default(),
        PreviewCamera,
        DespawnOnExit(Screen::Character),
    ));

    commands.spawn((
        DirectionalLight { illuminance: light_consts::lux::OVERCAST_DAY, shadow_maps_enabled: false, ..default() },
        Transform::from_xyz(2.0, 4.0, -4.0).looking_at(Vec3::ZERO, Vec3::Y),
        DespawnOnExit(Screen::Character),
    ));

    let turntable = commands.spawn((Transform::default(), Visibility::default(), PreviewTurntable, DespawnOnExit(Screen::Character))).id();
    let pose = HumanPose(std::array::from_fn(|i| match i {
        4..=6 => Quat::from_rotation_z(-20.0f32.to_radians()),
        7..=9 => Quat::from_rotation_z(20.0f32.to_radians()),
        _ => Quat::IDENTITY,
    }));

    commands.spawn((human_look(&look), pose, PreviewHuman, ChildOf(turntable)));
}

fn human_look(look: &PlayerLook) -> HumanLook {
    let column = |v: i32| v.max(0) as u32;

    HumanLook {
        gender: column(look.gender),
        head: column(look.head),
        hair: column(look.hair),
        skin_colour: column(look.skin),
        hair_colour: column(look.hair_colour),
        eye_colour: column(look.eyes),
        suit_colour: SUIT_COLOUR,
        ..default()
    }
}

fn wear_player_look(look: Res<PlayerLook>, mut config: ResMut<Config>, mut humans: Query<&mut HumanLook, With<PreviewHuman>>) {
    if !look.is_changed() {
        return;
    }

    if PlayerLook::from(&*config) != *look {
        config.player_gender = look.gender;
        config.player_head = look.head;
        config.player_eye_color = look.eyes;
        config.player_hair_style = look.hair;
        config.player_hair_color = look.hair_colour;
        config.player_skin_color = look.skin;

        if let Err(e) = config.save(&config_path()) {
            warn!("config.txt not saved: {e}");
        }
    }

    let wanted = human_look(&look);

    for mut human in &mut humans {
        if *human != wanted {
            *human = wanted;
        }
    }
}

fn drag_preview(drag: On<PointerDrag>, mut view: ResMut<PreviewView>) {
    view.turn += drag.delta.x * TURN_PER_PIXEL;
    view.pitch = (view.pitch + drag.delta.y * TURN_PER_PIXEL).clamp(-PITCH_LIMIT, PITCH_LIMIT);
}

fn zoom_preview(mut scroll: On<PointerScroll>, mut view: ResMut<PreviewView>) {
    scroll.propagate(false);
    let notches = match scroll.unit {
        MouseScrollUnit::Line => scroll.y,
        MouseScrollUnit::Pixel => scroll.y / PIXELS_PER_NOTCH,
    };
    view.distance = (view.distance * ZOOM_PER_NOTCH.powf(notches)).clamp(NEAREST, FARTHEST);
}

fn apply_view(
    view: Res<PreviewView>,
    mut turntables: Query<&mut Transform, (With<PreviewTurntable>, Without<PreviewCamera>, Without<PreviewHuman>)>,
    mut humans: Query<(&mut Transform, &HumanExtent), (With<PreviewHuman>, Without<PreviewCamera>)>,
    mut cameras: Query<&mut Transform, With<PreviewCamera>>,
) {
    for mut transform in &mut turntables {
        transform.rotation = Quat::from_rotation_y(view.turn);
    }

    for (mut transform, extent) in &mut humans {
        if transform.translation.y != -extent.lowest {
            transform.translation.y = -extent.lowest;
        }
    }

    let near = ((START_DISTANCE - view.distance) / (START_DISTANCE - NEAREST)).clamp(0.0, 1.0).sqrt();
    let target = Vec3::Y * (BODY_HEIGHT + (FACE_HEIGHT - BODY_HEIGHT) * near);
    let eye = target + Vec3::new(0.0, view.pitch.sin(), -view.pitch.cos()) * view.distance;

    for mut transform in &mut cameras {
        *transform = Transform::from_translation(eye).looking_at(target, Vec3::Y);
    }
}
