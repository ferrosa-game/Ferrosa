mod canvas;
mod picture;
mod screen;

use bevy::{prelude::*, ui::RelativeCursorPosition};

use super::{
    GAME_SCREEN,
    computer::AtComputer,
    escape_menu::{GameMenu, OVERLAY_LAYER},
    hud::{Stick, align, rect, spawn_screen},
    netgame::NetGame,
};
use crate::menu::Screen;

const SPAN: f32 = 256.0;
const VIEW_SIZE: f32 = 544.0;
const MINI_SIZE: f32 = 176.0;
const MINI_AT: Vec2 = Vec2::new(GAME_SCREEN.x - 16.0 - MINI_SIZE, 36.0);

const FRAME: Color = Color::srgb(0.45, 0.47, 0.5);
const MAP_GROUND: Color = Color::srgb(0.05, 0.05, 0.06);
const BORDER: f32 = 2.0;
const RING_WIDTH: f32 = 2.0;
const MARKER_SIZE: f32 = 12.0;
const MARKER_EDGE: f32 = 2.0;
const MARKER_COLOUR: [f32; 4] = [1.0, 0.15, 0.15, 1.0];
const DROP_COLOUR: [f32; 4] = [0.2, 1.0, 0.3, 1.0];
const SPOT_COLOUR: [f32; 4] = [0.75, 0.75, 0.75, 1.0];
const BANK_LABEL: [f32; 4] = [0.3, 0.85, 0.3, 1.0];

#[derive(Resource)]
struct MapPicture {
    image: Handle<Image>,
    origin: Vec2,
    scale: f32,
    round: bool,
}

impl MapPicture {
    fn on_map(&self, world: Vec2) -> Vec2 {
        (world - self.origin) * self.scale
    }
}

#[derive(Resource, Default)]
struct MinimapOpen(bool);

#[derive(Resource)]
struct MapView {
    zoom: f32,
    offset: Vec2,
    drag: Option<Vec2>,
}

impl Default for MapView {
    fn default() -> Self {
        Self { zoom: fit_zoom(), offset: Vec2::ZERO, drag: None }
    }
}

#[derive(Component, Clone, Copy, PartialEq)]
enum Viewport {
    Full,
    Mini,
}

#[derive(Component, Default)]
struct Canvas(Option<(f32, Vec<canvas::Mark>)>);

#[derive(Component)]
struct MiniScreen;

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MinimapOpen>()
            .init_resource::<MapView>()
            .add_systems(OnEnter(Screen::InGame), (picture::prepare_picture, spawn_minimap).chain())
            .add_systems(OnEnter(GameMenu::Map), screen::spawn_map_screen)
            .add_systems(Update, (toggle_maps, picture::finish_picture).run_if(in_state(Screen::InGame)).run_if(resource_exists::<MapPicture>))
            .add_systems(Update, (screen::steer_map, screen::fill_briefing).run_if(in_state(GameMenu::Map)).run_if(resource_exists::<NetGame>))
            .add_systems(
                Update,
                (show_minimap, canvas::draw_canvases, canvas::place_canvases, canvas::place_pointers).chain().run_if(in_state(Screen::InGame)).run_if(resource_exists::<MapPicture>),
            )
            .add_systems(OnExit(Screen::InGame), |mut commands: Commands, mut open: ResMut<MinimapOpen>, mut view: ResMut<MapView>| {
                open.0 = false;
                *view = MapView::default();
                commands.remove_resource::<MapPicture>();
            });
    }
}

fn fit_zoom() -> f32 {
    VIEW_SIZE / SPAN
}

fn viewport(commands: &mut Commands, parent: Entity, at: Vec2, size: f32, kind: Viewport) -> Entity {
    let node = Node { border: UiRect::all(px(BORDER)), overflow: Overflow::clip(), ..rect(align::TOP_LEFT, at, Vec2::splat(size)) };
    let view = commands.spawn((node, BackgroundColor(MAP_GROUND), BorderColor::all(FRAME), kind, RelativeCursorPosition::default(), ChildOf(parent))).id();

    commands.spawn((Node { position_type: PositionType::Absolute, ..default() }, Canvas::default(), Pickable::IGNORE, ChildOf(view)));

    view
}

fn spawn_minimap(mut commands: Commands) {
    let screen = spawn_screen(&mut commands, MiniScreen, Stick::TopRight);
    commands.entity(screen).insert(GlobalZIndex(OVERLAY_LAYER));
    viewport(&mut commands, screen, MINI_AT, MINI_SIZE, Viewport::Mini);
}

fn toggle_maps(keys: Res<ButtonInput<KeyCode>>, menu: Res<State<GameMenu>>, at_computer: Res<AtComputer>, mut next: ResMut<NextState<GameMenu>>, mut minimap: ResMut<MinimapOpen>) {
    match menu.get() {
        GameMenu::Closed | GameMenu::PlayerMenu if !at_computer.typing() => {
            if keys.just_pressed(KeyCode::KeyC) {
                next.set(GameMenu::Map);
            }

            if keys.just_pressed(KeyCode::KeyM) {
                minimap.0 = !minimap.0;
            }
        }
        GameMenu::Map if keys.any_just_pressed([KeyCode::KeyC, KeyCode::Escape]) => next.set(GameMenu::Closed),
        _ => {}
    }
}

fn player_on_map(net: &NetGame, picture: &MapPicture) -> Option<Vec2> {
    let p = net.me()?;
    let h = usize::try_from(p.human).ok().and_then(|h| net.humans.get(h)).filter(|h| h.active)?;

    Some(picture.on_map(Vec2::new(h.track.pos.x, h.track.pos.z)))
}

fn show_minimap(minimap: Res<MinimapOpen>, menu: Res<State<GameMenu>>, net: Option<Res<NetGame>>, mut screens: Query<&mut Visibility, With<MiniScreen>>) {
    let shown = minimap.0 && *menu.get() != GameMenu::Map && net.is_some_and(|n| n.me().is_some());

    for mut v in &mut screens {
        v.set_if_neq(if shown { Visibility::Inherited } else { Visibility::Hidden });
    }
}
