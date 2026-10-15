use bevy::{
    prelude::*,
    ui_widgets::Activate,
};

use crate::{
    config::Config,
    masterserver::{MasterAuth, auth_status}, menu::character::{character, PlayerLook}, steam::Steam, ui::{self, Shortcut},
};

pub mod character;
pub mod join;
pub mod options;
pub mod servers;

#[derive(States, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Screen {
    #[default]
    Main,
    ServerList,
    Options,
    HostGame,
    Character,
    Connecting,
}

pub const MENU_BACKGROUND: Color = Color::srgb(0.035, 0.063, 0.082);

const MENU_BUTTON: Vec2 = Vec2::new(320.0, 44.0);
const HEADING: Color = Color::srgb(0.75, 0.75, 0.75);

#[derive(Component, Clone, Copy, Debug)]
enum Action {
    Practice,
    Goto(Screen),
    Exit,
}

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        let look = app.world().get_resource::<Config>().map_or_else(PlayerLook::default, PlayerLook::from);
        app.init_state::<Screen>()
            .insert_resource(look)
            .add_plugins((character::CharacterPreviewPlugin, servers::ServerBrowserPlugin, options::OptionsPlugin, join::JoinPlugin))
            .add_systems(OnEnter(Screen::Main), main_menu)
            .add_systems(OnEnter(Screen::Character), character)
            .add_systems(OnEnter(Screen::HostGame), |commands: Commands| placeholder(commands, Screen::HostGame, "Start Game"))
            .add_systems(Update, update_auth_status)
            .add_observer(on_activate);
    }
}

fn page(commands: &mut Commands, screen: Screen) -> Entity {
    commands
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
            DespawnOnExit(screen),
        ))
        .id()
}

fn text(commands: &mut Commands, parent: Entity, s: &str, size: f32, colour: Color) {
    commands.spawn((Text::new(s), TextFont::from_font_size(size), TextColor(colour), ChildOf(parent)));
}

fn gap(commands: &mut Commands, parent: Entity, height: f32) {
    commands.spawn((Node { height: px(height), ..default() }, ChildOf(parent)));
}

fn button(commands: &mut Commands, parent: Entity, label: &str, action: Action, shortcut: Option<KeyCode>) {
    let b = ui::button(commands, parent, label, MENU_BUTTON);

    commands.entity(b).insert(action);

    if let Some(key) = shortcut {
        commands.entity(b).insert(Shortcut(key));
    }
}

fn main_menu(mut commands: Commands) {
    let p = page(&mut commands, Screen::Main);

    text(&mut commands, p, "Ferrosa", 48.0, Color::WHITE);
    text(&mut commands, p, &format!("ALPHA {}{}", 38, (b'a' + 5) as char), 18.0, HEADING);

    gap(&mut commands, p, 32.0);

    let buttons = [
        ("Practice", Action::Practice, Some(KeyCode::KeyD)),
        ("Start Game", Action::Goto(Screen::HostGame), Some(KeyCode::KeyS)),
        ("Join Game", Action::Goto(Screen::ServerList), Some(KeyCode::KeyJ)),
        ("Character", Action::Goto(Screen::Character), Some(KeyCode::KeyC)),
        ("Options", Action::Goto(Screen::Options), Some(KeyCode::KeyO)),
        ("Exit", Action::Exit, Some(KeyCode::Escape)),
    ];

    for (label, action, key) in buttons {
        button(&mut commands, p, label, action, key);
    }

    commands.spawn((
        Node { position_type: PositionType::Absolute, left: px(16), bottom: px(12), ..default() },
        Text::default(),
        TextFont::from_font_size(16.0),
        TextColor(HEADING),
        AuthStatusText,
        ChildOf(p),
    ));
}

fn back_button(commands: &mut Commands, parent: Entity) {
    gap(commands, parent, 24.0);

    button(commands, parent, "Back", Action::Goto(Screen::Main), Some(KeyCode::Escape));
}

fn placeholder(mut commands: Commands, screen: Screen, title: &str) {
    let p = page(&mut commands, screen);
    text(&mut commands, p, title, 32.0, Color::WHITE);
    back_button(&mut commands, p);
}

fn on_activate(activate: On<Activate>, actions: Query<&Action>, mut next: ResMut<NextState<Screen>>, mut commands: Commands) {
    let Ok(&action) = actions.get(activate.entity) else { return };
    match action {
        // TODO: Practice starts a local game, which needs a server alongside the client.
        Action::Practice => {}
        Action::Goto(screen) => next.set(screen),
        Action::Exit => {
            commands.write_message(AppExit::Success);
        }
    }
}

#[derive(Component)]
struct AuthStatusText;

fn update_auth_status(master: Option<Res<MasterAuth>>, steam: Option<Res<Steam>>, mut texts: Query<&mut Text, With<AuthStatusText>>) {
    let Some(steam) = steam else { return };

    let status = auth_status(master.as_deref(), &steam);

    for mut text in &mut texts {
        if text.0 != status {
            text.0.clone_from(&status);
        }
    }
}
