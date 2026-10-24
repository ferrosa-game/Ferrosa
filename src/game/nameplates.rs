use bevy::prelude::*;
use ferrosa_world::skeleton::BoneId;

use super::{
    Level,
    camera::GameCamera,
    computer::AtComputer,
    escape_menu::GameMenu,
    first_person::FirstPerson,
    hud::{align, anchored, colour, text},
    human_sync::{NetHuman, bone_at},
    netgame::NetGame,
    smoothing::Smoothing,
    voice::line_of_sight,
};
use crate::{character_model::HumanPose, fonts::GameFonts, menu::Screen, names::{TEAM_COLOURS, phone_number}};

#[derive(Component)]
struct Nameplates;

#[derive(Resource)]
struct ShowNames(bool);

impl Default for ShowNames {
    fn default() -> Self {
        Self(true)
    }
}

pub struct NameplatesPlugin;

impl Plugin for NameplatesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShowNames>()
            .add_systems(OnEnter(Screen::InGame), spawn_layer)
            .add_systems(Update, (toggle_names, show_nameplates).chain().after(Smoothing::Glide).run_if(in_state(Screen::InGame)).run_if(resource_exists::<NetGame>));
    }
}

fn toggle_names(keys: Res<ButtonInput<KeyCode>>, menu: Option<Res<State<GameMenu>>>, at_computer: Res<AtComputer>, mut showing: ResMut<ShowNames>) {
    let typing = menu.is_some_and(|m| *m.get() == GameMenu::Chat) || at_computer.typing();

    if keys.any_just_pressed([KeyCode::AltLeft, KeyCode::AltRight]) && !typing {
        showing.0 = !showing.0;
    }
}

fn spawn_layer(mut commands: Commands) {
    let full = Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() };

    commands.spawn((full, Nameplates, Pickable::IGNORE, DespawnOnExit(Screen::InGame)));
}

fn show_nameplates(
    mut commands: Commands,
    net: Res<NetGame>,
    fonts: Res<GameFonts>,
    scale: Res<UiScale>,
    level: Option<Res<Level>>,
    showing: Res<ShowNames>,
    first_person: Res<FirstPerson>,
    camera: Single<(&Camera, &GlobalTransform), With<GameCamera>>,
    humans: Query<(&NetHuman, &Transform, &HumanPose)>,
    layer: Single<(Entity, Option<&Children>), With<Nameplates>>,
) {
    let (layer, children) = *layer;

    children.into_iter().flatten().for_each(|&c| commands.entity(c).despawn());

    if !showing.0 {
        return;
    }

    let (camera, view) = *camera;
    let eye = view.translation();
    let own = net.me().map(|p| p.human).filter(|_| first_person.active);

    for (nh, at, pose) in &humans {
        let Some(human) = net.humans.get(nh.0) else { continue };

        if !human.active || !human.alive || Some(nh.0 as i32) == own {
            continue;
        }

        let Some(player) = usize::try_from(human.player).ok().and_then(|p| net.players.get(p)) else { continue };

        if !player.active || player.name.is_empty() {
            continue;
        }

        let head = bone_at(at.translation, pose, BoneId::Head.index()).0 + Vec3::Y * 0.75;
        let dist_away = head.distance(eye);

        if dist_away > 64.0 || level.as_ref().is_some_and(|l| line_of_sight(l, head, eye) < 1.0) {
            continue;
        }

        let Ok(on_screen) = camera.world_to_viewport(view, head) else { continue };

        let size = (14.0 * 6.0 / dist_away).clamp(7.0, 14.0);
        let fade = ((64.0 - dist_away) / (64.0 * 0.25)).clamp(0.0, 1.0);
        let [r, g, b] = TEAM_COLOURS.get(player.team as usize).copied().unwrap_or(TEAM_COLOURS[6]);
        let place = on_screen / scale.0;

        let (node, shift) = anchored(align::CENTRE_X, place);
        let column = commands
            .spawn((Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..node }, shift, Pickable::IGNORE, ChildOf(layer)))
            .id();

        let shade = colour([r, g, b, fade]);
        let dim = colour([r * 0.8, g * 0.8, b * 0.8, fade]);

        commands.spawn((text(player.name.clone(), &fonts.rockwell, size, shade), TextShadow { offset: Vec2::splat(size / 16.0), color: Color::srgba(0.0, 0.0, 0.0, fade) }, ChildOf(column)));
        commands.spawn((text(phone_number(player.phone_number as u32), &fonts.rockwell, size * 0.75, dim), TextShadow { offset: Vec2::splat(size / 16.0), color: Color::srgba(0.0, 0.0, 0.0, fade) }, ChildOf(column)));
    }
}
