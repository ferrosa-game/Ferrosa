use bevy::{
    picking::{events::PointerClick, hover::Hovered},
    prelude::*,
    ui_widgets::Activate,
};

use super::{
    escape_menu::{GameMenu, MENU_LAYER},
    netgame::{NetGame, entities::Player},
};
use crate::{
    fonts::GameFonts,
    menu::Screen,
    ui::{self, Shortcut},
};

const SHADE: Color = Color::srgba(0.0, 0.0, 0.0, 0.5);
const FRAME: Color = Color::srgb(0.75, 0.75, 0.75);
const INSIDE: Color = Color::srgb(0.25, 0.25, 0.25);
const INSIDE_HOVERED: Color = Color::srgb(0.5, 0.5, 0.5);
const SCREEN: Vec2 = Vec2::new(1024.0, 576.0);
const BACK_AT: Vec2 = Vec2::new(16.0, 16.0);
const BACK: Vec2 = Vec2::new(192.0, 28.0);
const LIST_AT: Vec2 = Vec2::new(192.0, 64.0);
const LIST: Vec2 = Vec2::new(640.0, 384.0);
const ENTRY: Vec2 = Vec2::new(160.0, 48.0);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Voice {
    Speaker,
    Talking,
    Muted,
}

impl Voice {
    pub fn of(p: &Player) -> Self {
        if p.muted {
            Voice::Muted
        } else if p.is_speaking {
            Voice::Talking
        } else {
            Voice::Speaker
        }
    }
}

#[derive(Resource)]
pub struct VoiceIcons {
    speaker: Handle<Image>,
    talking: Handle<Image>,
    muted: Handle<Image>,
}

impl VoiceIcons {
    pub fn get(&self, voice: Voice) -> Handle<Image> {
        match voice {
            Voice::Speaker => self.speaker.clone(),
            Voice::Talking => self.talking.clone(),
            Voice::Muted => self.muted.clone(),
        }
    }
}

#[derive(Component, Clone, Copy)]
pub struct MuteToggle(pub usize);

#[derive(Component)]
struct MuteBack;

#[derive(Component)]
struct MuteList;

#[derive(Component)]
struct MuteEntry;

#[derive(Resource, Default, PartialEq)]
struct Entries(Vec<(usize, String, i32, bool, bool)>);

pub struct MutePlugin;

impl Plugin for MutePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Entries>()
            .add_systems(Startup, load_icons)
            .add_systems(OnEnter(GameMenu::Mute), spawn_mute_page)
            .add_systems(Update, (fill_mute_list, shade_entries).chain().run_if(in_state(GameMenu::Mute)).run_if(resource_exists::<NetGame>))
            .add_systems(OnExit(GameMenu::Mute), |mut entries: ResMut<Entries>| entries.0.clear())
            .add_observer(toggle_on_click)
            .add_observer(go_back);
    }
}

fn load_icons(mut commands: Commands, assets: Res<AssetServer>) {
    let icon = |name: &str| assets.load(format!("icons/icon_{name}.png"));

    commands.insert_resource(VoiceIcons { speaker: icon("speaker"), talking: icon("talking"), muted: icon("muted") });
}

pub fn voice_icon(commands: &mut Commands, parent: Entity, icons: &VoiceIcons, voice: Voice, size: f32, toggle: Option<usize>) -> Entity {
    let node = Node { width: px(size), height: px(size), flex_shrink: 0.0, ..default() };
    let icon = commands.spawn((node, ImageNode::new(icons.get(voice)), ChildOf(parent))).id();

    match toggle {
        Some(k) => commands.entity(icon).insert(MuteToggle(k)),
        None => commands.entity(icon).insert(Pickable::IGNORE),
    };

    icon
}

fn toggle_on_click(click: On<PointerClick>, toggles: Query<&MuteToggle>, net: Option<ResMut<NetGame>>) {
    let (Ok(&MuteToggle(k)), Some(mut net)) = (toggles.get(click.entity), net) else { return };

    net.toggle_mute(k);
}

fn go_back(activate: On<Activate>, backs: Query<(), With<MuteBack>>, mut menu: ResMut<NextState<GameMenu>>) {
    if backs.contains(activate.entity) {
        menu.set(GameMenu::Open);
    }
}

fn spawn_mute_page(mut commands: Commands) {
    let full = Node { width: percent(100), height: percent(100), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() };
    let page = commands.spawn((full, BackgroundColor(SHADE), GlobalZIndex(MENU_LAYER), DespawnOnExit(GameMenu::Mute), DespawnOnExit(Screen::InGame))).id();
    let screen = commands.spawn((Node { width: px(SCREEN.x), height: px(SCREEN.y), ..default() }, Pickable::IGNORE, ChildOf(page))).id();

    let back_holder = Node { position_type: PositionType::Absolute, left: px(BACK_AT.x), top: px(BACK_AT.y), ..default() };
    let holder = commands.spawn((back_holder, Pickable::IGNORE, ChildOf(screen))).id();
    let back = ui::button(&mut commands, holder, "Back", BACK);
    commands.entity(back).insert((MuteBack, Shortcut(KeyCode::Escape)));

    let list = Node {
        position_type: PositionType::Absolute,
        left: px(LIST_AT.x),
        top: px(LIST_AT.y),
        width: px(LIST.x),
        height: px(LIST.y),
        flex_direction: FlexDirection::Row,
        flex_wrap: FlexWrap::Wrap,
        align_content: AlignContent::FlexStart,
        overflow: Overflow::scroll_y(),
        ..default()
    };
    commands.spawn((list, MuteList, ChildOf(screen)));
}

fn line(commands: &mut Commands, parent: Entity, s: &str, font: &Handle<Font>, size: f32) {
    commands.spawn((Text::new(s), TextFont::from_font_size(size).with_font(font.clone()), TextColor(Color::WHITE), Pickable::IGNORE, ChildOf(parent)));
}

fn fill_mute_list(mut commands: Commands, net: Res<NetGame>, fonts: Res<GameFonts>, mut shown: ResMut<Entries>, list: Single<Entity, With<MuteList>>) {
    let now = Entries(
        net.players
            .iter()
            .enumerate()
            .filter(|&(k, p)| p.active && !p.is_bot && k as i32 != net.player)
            .map(|(k, p)| (k, p.name.clone(), p.phone_number, p.is_speaking, p.muted))
            .collect(),
    );

    if *shown == now {
        return;
    }

    commands.entity(*list).despawn_children();

    for (k, name, number, speaking, muted) in &now.0 {
        let node = Node {
            width: px(ENTRY.x),
            height: px(ENTRY.y),
            border: UiRect::all(px(2)),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            ..default()
        };
        let entry = commands.spawn((node, BackgroundColor(INSIDE), BorderColor::all(FRAME), Hovered::default(), MuteEntry, MuteToggle(*k), ChildOf(*list))).id();
        let number = *number as u32;

        line(&mut commands, entry, name, &fonts.rockwell, if *speaking && !*muted { 16.0 } else { 12.0 });
        line(&mut commands, entry, &format!("{:03}-{:04}", number / 10000, number % 10000), &fonts.rockwell, 12.0);

        if *muted {
            line(&mut commands, entry, "MUTED", &fonts.rockwell, 16.0);
        }
    }

    *shown = now;
}

fn shade_entries(mut entries: Query<(&Hovered, &mut BackgroundColor), With<MuteEntry>>) {
    for (hovered, mut background) in &mut entries {
        let colour = if hovered.get() { INSIDE_HOVERED } else { INSIDE };

        if background.0 != colour {
            background.0 = colour;
        }
    }
}
