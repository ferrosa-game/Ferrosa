mod aim;
mod gauges;
mod menu_line;
mod phone;
mod status;
mod timer;

use bevy::prelude::*;
use ferrosa_protocol::clientbound::game::{GameState, ItemKind, MenuType, OwnHumanData};

use super::{
    GAME_SCREEN,
    escape_menu::BACKDROP_LAYER,
    netgame::{NetGame, entities::{Human, Player}},
};
use crate::{
    fonts::GameFonts,
    menu::{MENU_BACKGROUND, Screen},
};

pub use phone::DIALLING;

const PAIN_PER_SHADE: f32 = 64.0;
const DARKEST: f32 = 0.75;
const DAMAGE_LAYER: i32 = 1;

pub mod align {
    pub const CENTRE_X: u32 = 1;
    pub const RIGHT: u32 = 2;
    pub const CENTRE_Y: u32 = 4;
    pub const BOTTOM: u32 = 8;
    pub const TOP_LEFT: u32 = 0;
    pub const CENTRE: u32 = CENTRE_X | CENTRE_Y;
    pub const BOTTOM_CENTRE: u32 = CENTRE_X | BOTTOM;
}

#[derive(Component)]
pub struct HudScreen;

#[derive(Clone, Copy)]
pub enum Stick {
    Centre,
    Top,
    Bottom,
    BottomLeft,
    TopLeft,
    TopRight,
}

#[derive(Component)]
struct DamageFade;

#[derive(Component)]
struct Backdrop;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::InGame), spawn_hud).add_systems(
            Update,
            (
                show_hud,
                (
                    status::show_status,
                    menu_line::show_menu_line,
                    phone::show_phone_help,
                    aim::show_crosshair,
                    aim::show_aim,
                    gauges::show_gauges,
                    fade_for_damage,
                    timer::show_timer,
                    show_backdrop,
                )
                    .run_if(resource_exists::<NetGame>),
            )
                .chain()
                .run_if(in_state(Screen::InGame)),
        );
    }
}

pub fn own(net: &NetGame) -> Option<(&Player, &Human, &OwnHumanData)> {
    let p = net.me()?;
    let h = usize::try_from(p.human).ok().and_then(|h| net.humans.get(h))?;

    Some((p, h, net.own_human.as_ref()?))
}

pub fn in_hand(net: &NetGame, h: &Human, hand: usize) -> Option<(usize, ItemKind)> {
    let item = usize::try_from(*h.inventory[hand].first()?).ok()?;

    Some((item, net.items.get(item)?.kind?))
}

pub fn is_phone(kind: ItemKind) -> bool {
    matches!(kind, ItemKind::Phone | ItemKind::PhonePay)
}

pub fn colour([r, g, b, a]: [f32; 4]) -> Color {
    Color::srgba(r, g, b, a)
}

pub fn rect(flags: u32, at: Vec2, size: Vec2) -> Node {
    let x = at.x - if flags & align::CENTRE_X != 0 { size.x * 0.5 } else { 0.0 } - if flags & align::RIGHT != 0 { size.x } else { 0.0 };
    let y = at.y - if flags & align::CENTRE_Y != 0 { size.y * 0.5 } else { 0.0 } - if flags & align::BOTTOM != 0 { size.y } else { 0.0 };

    Node { position_type: PositionType::Absolute, left: px(x), top: px(y), width: px(size.x), height: px(size.y), ..default() }
}

pub fn anchored(flags: u32, at: Vec2) -> (Node, UiTransform) {
    let x = if flags & align::CENTRE_X != 0 { -50.0 } else if flags & align::RIGHT != 0 { -100.0 } else { 0.0 };
    let y = if flags & align::CENTRE_Y != 0 { -50.0 } else if flags & align::BOTTOM != 0 { -100.0 } else { 0.0 };
    let node = Node { position_type: PositionType::Absolute, left: px(at.x), top: px(at.y), ..default() };

    (node, UiTransform::from_translation(Val2::new(percent(x), percent(y))))
}

pub fn text(s: impl Into<String>, font: &Handle<Font>, size: f32, colour: Color) -> impl Bundle {
    (Text::new(s), TextFont::from_font_size(size).with_font(font.clone()), TextColor(colour), Pickable::IGNORE)
}

pub fn line(a: Vec2, b: Vec2, width: f32, round: bool) -> (Node, UiTransform) {
    let d = b - a;
    let length = d.length() + if round { width } else { 0.0 };
    let mid = (a + b) * 0.5;
    let radius = if round { px(width * 0.5) } else { px(0) };

    let node = Node {
        position_type: PositionType::Absolute,
        left: px(mid.x - length * 0.5),
        top: px(mid.y - width * 0.5),
        width: px(length),
        height: px(width),
        border_radius: BorderRadius::all(radius),
        ..default()
    };

    (node, UiTransform::from_rotation(Rot2::radians(d.y.atan2(d.x))))
}

pub fn spawn_screen(commands: &mut Commands, marker: impl Bundle, stick: Stick) -> Entity {
    let (across, down) = match stick {
        Stick::Centre => (JustifyContent::Center, AlignItems::Center),
        Stick::Bottom => (JustifyContent::Center, AlignItems::FlexEnd),
        Stick::BottomLeft => (JustifyContent::FlexStart, AlignItems::FlexEnd),
        Stick::TopLeft => (JustifyContent::FlexStart, AlignItems::FlexStart),
        Stick::Top => (JustifyContent::Center, AlignItems::FlexStart),
        Stick::TopRight => (JustifyContent::FlexEnd, AlignItems::FlexStart),
    };
    let full = Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), justify_content: across, align_items: down, ..default() };
    let window = commands.spawn((full, Pickable::IGNORE, DespawnOnExit(Screen::InGame))).id();
    let screen = Node { width: px(GAME_SCREEN.x), height: px(GAME_SCREEN.y), ..default() };

    commands.spawn((screen, marker, Pickable::IGNORE, Visibility::Hidden, ChildOf(window))).id()
}

fn shown(visibility: &mut Mut<Visibility>, show: bool) {
    visibility.set_if_neq(if show { Visibility::Inherited } else { Visibility::Hidden });
}

fn set_text(t: &mut Text, v: &mut Mut<Visibility>, s: Option<String>) {
    shown(v, s.is_some());

    if let Some(s) = s
        && t.0 != s
    {
        t.0 = s;
    }
}

fn spawn_hud(mut commands: Commands, fonts: Res<GameFonts>) {
    let centre = spawn_screen(&mut commands, HudScreen, Stick::Centre);
    let bottom = spawn_screen(&mut commands, HudScreen, Stick::Bottom);
    let bottom_left = spawn_screen(&mut commands, HudScreen, Stick::BottomLeft);
    let top_left = spawn_screen(&mut commands, HudScreen, Stick::TopLeft);
    let top = spawn_screen(&mut commands, HudScreen, Stick::Top);

    status::spawn(&mut commands, centre, bottom_left, &fonts);
    menu_line::spawn(&mut commands, top_left, top, &fonts);
    phone::spawn(&mut commands, centre);
    aim::spawn(&mut commands, centre);
    gauges::spawn(&mut commands, bottom, &fonts.rockwell);
    timer::spawn(&mut commands, &fonts);

    let backdrop = Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() };
    commands.spawn((backdrop, Backdrop, BackgroundColor(MENU_BACKGROUND), Visibility::Hidden, GlobalZIndex(BACKDROP_LAYER), Pickable::IGNORE, DespawnOnExit(Screen::InGame)));

    let full = Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() };
    commands.spawn((full, DamageFade, BackgroundColor(Color::NONE), GlobalZIndex(DAMAGE_LAYER), Pickable::IGNORE, DespawnOnExit(Screen::InGame)));
}

fn show_hud(net: Option<Res<NetGame>>, mut screens: Query<&mut Visibility, With<HudScreen>>) {
    let ours = net.is_some_and(|n| own(&n).is_some());

    screens.iter_mut().for_each(|mut v| shown(&mut v, ours));
}

fn show_backdrop(net: Res<NetGame>, mut backdrop: Single<&mut Visibility, With<Backdrop>>) {
    let lobby = net.me().is_some_and(|p| p.menu == MenuType::Lobby);

    shown(&mut backdrop, lobby || net.game_state == GameState::Restarting);
}

fn fade_for_damage(net: Res<NetGame>, mut fade: Single<&mut BackgroundColor, With<DamageFade>>) {
    let dark = own(&net).map_or(0.0, |(_, _, own)| (own.pain.max(0) as f32 / PAIN_PER_SHADE).min(DARKEST));
    let colour = Color::BLACK.with_alpha(dark);

    if fade.0 != colour {
        fade.0 = colour;
    }
}
