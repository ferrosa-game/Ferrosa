use bevy::{prelude::*, ui_widgets::Activate};

use super::netgame::NetGame;
use crate::{
    audio::AudioDevices,
    config::Config,
    menu::{Screen, options::spawn_options},
    net::ClientSocket,
    ui::{self, Shortcut},
};

const BUTTON: Vec2 = Vec2::new(240.0, 32.0);
pub const BACKDROP_LAYER: i32 = 2;
pub const PLAYER_MENU_LAYER: i32 = 3;
pub const ROSTER_LAYER: i32 = 4;
pub const OVERLAY_LAYER: i32 = 5;
pub const MENU_LAYER: i32 = 6;
const BACK_BUTTON: Vec2 = Vec2::new(320.0, 44.0);
const OPTIONS_SHADE: Color = Color::srgba(0.035, 0.063, 0.082, 0.9);

#[derive(SubStates, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[source(Screen = Screen::InGame)]
pub enum GameMenu {
    #[default]
    Closed,
    Open,
    Options,
    PlayerMenu,
    Chat,
    Mute,
    Admin,
    Map,
}

#[derive(Component, Clone, Copy)]
enum MenuButton {
    ReturnToGame,
    Mute,
    Admin,
    Options,
    ExitGame,
    Back,
}

pub struct EscapeMenuPlugin;

impl Plugin for EscapeMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_sub_state::<GameMenu>()
            .add_systems(Update, open_on_escape.run_if(|menu: Option<Res<State<GameMenu>>>| menu.is_some_and(|m| matches!(m.get(), GameMenu::Closed | GameMenu::PlayerMenu))))
            .add_systems(OnEnter(GameMenu::Open), spawn_menu)
            .add_systems(OnEnter(GameMenu::Options), spawn_options_page)
            .add_observer(on_button);
    }
}

fn open_on_escape(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<GameMenu>>) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(GameMenu::Open);
    }
}

fn menu_button(commands: &mut Commands, parent: Entity, label: &str, action: MenuButton, key: KeyCode, size: Vec2) {
    let b = ui::button(commands, parent, label, size);

    commands.entity(b).insert((action, Shortcut(key)));
}

fn spawn_menu(mut commands: Commands, net: Option<Res<NetGame>>) {
    let page = commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(8),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            GlobalZIndex(MENU_LAYER),
            DespawnOnExit(GameMenu::Open),
        ))
        .id();

    menu_button(&mut commands, page, "Return to Game", MenuButton::ReturnToGame, KeyCode::Escape, BUTTON);

    if let Some(me) = net.as_ref().and_then(|n| n.me()) {
        menu_button(&mut commands, page, "Mute Player", MenuButton::Mute, KeyCode::KeyM, BUTTON);

        if me.is_admin {
            menu_button(&mut commands, page, "Admin", MenuButton::Admin, KeyCode::KeyA, BUTTON);
        }
    }

    menu_button(&mut commands, page, "Options", MenuButton::Options, KeyCode::KeyO, BUTTON);
    menu_button(&mut commands, page, "Exit Game", MenuButton::ExitGame, KeyCode::KeyE, BUTTON);
}

fn spawn_options_page(mut commands: Commands, config: Res<Config>, devices: Res<AudioDevices>) {
    let page = spawn_options(&mut commands, &config, &devices);

    commands.entity(page).insert((BackgroundColor(OPTIONS_SHADE), GlobalZIndex(MENU_LAYER), DespawnOnExit(GameMenu::Options)));
    commands.spawn((Node { height: px(24.0), ..default() }, ChildOf(page)));

    menu_button(&mut commands, page, "Back", MenuButton::Back, KeyCode::Escape, BACK_BUTTON);
}

fn on_button(
    activate: On<Activate>,
    buttons: Query<&MenuButton>,
    mut commands: Commands,
    net: Option<Res<NetGame>>,
    mut socket: ResMut<ClientSocket>,
    mut menu: ResMut<NextState<GameMenu>>,
    mut screen: ResMut<NextState<Screen>>,
) {
    let Ok(&button) = buttons.get(activate.entity) else { return };

    match button {
        MenuButton::ReturnToGame => menu.set(GameMenu::Closed),
        MenuButton::Mute => menu.set(GameMenu::Mute),
        MenuButton::Admin => menu.set(GameMenu::Admin),
        MenuButton::Options => menu.set(GameMenu::Options),
        MenuButton::Back => menu.set(GameMenu::Open),
        MenuButton::ExitGame => {
            if let Some(net) = net {
                net.leave(&mut socket);
                commands.remove_resource::<NetGame>();
            }

            screen.set(Screen::Main);
        }
    }
}
