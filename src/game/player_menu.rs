use bevy::{prelude::*, ui::InteractionDisabled, ui_widgets::Activate};
use ferrosa_protocol::{
    GameMode, Team,
    clientbound::game::{ItemKind, MenuButton, MenuType},
};

use super::{GAME_SCREEN, escape_menu::{GameMenu, PLAYER_MENU_LAYER}, items::ItemTypes, netgame::NetGame};
use crate::{
    menu::MENU_BACKGROUND,
    ui::{self, Shortcut},
};

const TAB: i32 = 0x1000000;
const DISABLED: i32 = 0x100;
const TAB_BUTTON: Vec2 = Vec2::new(160.0, 24.0);
const BUTTON: Vec2 = Vec2::new(252.0, 40.0);
const SMALL_BUTTON: Vec2 = Vec2::new(252.0, 22.0);
const SPAWN_BUTTON: Vec2 = Vec2::new(240.0, 32.0);
const PRICE: Color = Color::srgb(0.6, 0.85, 0.6);

const WEAPONS_TAB: i32 = 0x80;
const AMMO_TAB: i32 = 0x81;
const EQUIPMENT_TAB: i32 = 0x82;
const VEHICLES_TAB: i32 = 0x83;
const STOCKS_TAB: i32 = 0x84;
const MANAGER_TAB: i32 = 0x85;
const WEAPONS: [(&str, ItemKind, i32); 5] =
    [("9mm", ItemKind::Pistol, 0), ("MP5", ItemKind::Mp5, 1), ("AK-47", ItemKind::Ak47, 2), ("M-16", ItemKind::M16, 3), ("Uzi", ItemKind::Uzi, 4)];
const AMMO: [(&str, ItemKind, i32); 5] = [
    ("9mm Magazine", ItemKind::PistolMag, 0),
    ("MP5 Magazine", ItemKind::Mp5Mag, 1),
    ("AK-47 Magazine", ItemKind::Ak47Mag, 2),
    ("M-16 Magazine", ItemKind::M16Mag, 3),
    ("Uzi Magazine", ItemKind::UziMag, 4),
];
const EQUIPMENT: [(&str, ItemKind, i32); 3] = [("Bandage", ItemKind::Bandage, 0), ("Briefcase", ItemKind::Briefcase, 7), ("Grenade", ItemKind::Grenade, 6)];
const SHARES: [i32; 4] = [1, 10, 100, 1000];
const ENTER_CITY: i32 = 1;
const RESPAWN_AT_BASE: i32 = 2;
const READY: i32 = 5;

#[derive(Component, Clone, Copy)]
pub struct Choice(pub i32);

#[derive(Component)]
struct Done;

#[derive(Component)]
struct PlayerMenuPanel;

#[derive(Clone, PartialEq)]
struct MenuView {
    mode: GameMode,
    menu: MenuType,
    team: Team,
    manager_tab: bool,
    can_spawn: bool,
    money: i32,
    buttons: Vec<MenuButton>,
}

#[derive(Resource, Default)]
struct ShownMenu(Option<MenuView>);

pub struct PlayerMenuPlugin;

impl Plugin for PlayerMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShownMenu>()
            .add_systems(Update, focus_player_menu.run_if(resource_exists::<NetGame>))
            .add_systems(Update, build_player_menu.run_if(resource_exists::<NetGame>).run_if(in_state(GameMenu::PlayerMenu)))
            .add_systems(OnExit(GameMenu::PlayerMenu), |mut shown: ResMut<ShownMenu>| shown.0 = None)
            .add_observer(on_choice);
    }
}

fn focus_player_menu(keys: Res<ButtonInput<KeyCode>>, net: Res<NetGame>, state: Option<Res<State<GameMenu>>>, mut next: ResMut<NextState<GameMenu>>) {
    let Some(state) = state else { return };
    let menu = net.me().map_or(MenuType::Empty, |p| p.menu);
    let forced = matches!(menu, MenuType::EnterCity | MenuType::Lobby);
    let tab = keys.just_pressed(KeyCode::Tab);

    match state.get() {
        GameMenu::Closed if forced || (tab && menu != MenuType::Empty) => next.set(GameMenu::PlayerMenu),
        GameMenu::PlayerMenu if menu == MenuType::Empty || (tab && !forced) => next.set(GameMenu::Closed),
        _ => {}
    }
}

fn build_player_menu(mut commands: Commands, net: Res<NetGame>, items: Option<Res<ItemTypes>>, mut shown: ResMut<ShownMenu>, panels: Query<Entity, With<PlayerMenuPanel>>) {
    let Some(p) = net.me() else { return };

    let view = MenuView {
        mode: net.sync.game_type,
        menu: p.menu,
        team: p.team,
        manager_tab: p.manager_tab,
        can_spawn: p.spawn_timer <= 0,
        money: p.money,
        buttons: p.menu_buttons.clone(),
    };

    if shown.0.as_ref() == Some(&view) {
        return;
    }

    panels.iter().for_each(|e| commands.entity(e).despawn());

    let background = if view.menu == MenuType::Lobby { MENU_BACKGROUND } else { Color::NONE };
    let panel = commands
        .spawn((Node { width: percent(100), height: percent(100), ..default() }, BackgroundColor(background), GlobalZIndex(PLAYER_MENU_LAYER), PlayerMenuPanel, DespawnOnExit(GameMenu::PlayerMenu)))
        .id();
    let price = |kind: ItemKind| items.as_ref().and_then(|t| t.0.get(kind as usize)).map_or(0, |t| t.price);

    match view.menu {
        MenuType::EnterCity => respawn_menu(&mut commands, panel, &view),
        MenuType::Lobby => lobby(&mut commands, panel),
        menu if is_corporation_page(menu) && view.mode == GameMode::World => server_buttons(&mut commands, panel, &view),
        menu if is_corporation_page(menu) => requisition(&mut commands, panel, &view, &price),
        // TODO: World's stock, store, car dealer and bank menus.
        _ => {}
    }

    shown.0 = Some(view);
}

fn is_corporation_page(menu: MenuType) -> bool {
    (MenuType::RoundCorpWeapons as u32..=MenuType::WorldCorpRequistion as u32).contains(&(menu as u32))
}

fn group(commands: &mut Commands, parent: Entity, x: f32, y: f32, direction: FlexDirection) -> Entity {
    let (left, top) = (percent(x / GAME_SCREEN.x * 100.0), percent(y / GAME_SCREEN.y * 100.0));
    let node = Node { position_type: PositionType::Absolute, left, top, flex_direction: direction, row_gap: px(4), column_gap: px(4), ..default() };

    commands.spawn((node, ChildOf(parent))).id()
}

fn choice(commands: &mut Commands, parent: Entity, label: &str, value: i32, size: Vec2) -> Entity {
    let b = ui::button(commands, parent, label, size);
    commands.entity(b).insert(Choice(value));
    b
}

fn with_price(commands: &mut Commands, button: Entity, amount: i32) {
    let node = Node { position_type: PositionType::Absolute, right: px(4), bottom: px(2), ..default() };

    commands.spawn((Text::new(format!("${amount}")), TextFont::from_font_size(12.0), TextColor(PRICE), node, ChildOf(button)));
}

fn disabled_if(commands: &mut Commands, button: Entity, disabled: bool) {
    if disabled {
        commands.entity(button).insert(InteractionDisabled);
    }
}

fn done_button(commands: &mut Commands, parent: Entity, x: f32, y: f32) {
    let column = group(commands, parent, x, y, FlexDirection::Column);
    let b = ui::button(commands, column, "Done", BUTTON);

    commands.entity(b).insert(Done);
}

fn respawn_menu(commands: &mut Commands, panel: Entity, view: &MenuView) {
    let column = group(commands, panel, 392.0, 304.0, FlexDirection::Column);

    if Team::CORPORATIONS.contains(&view.team) {
        let base = choice(commands, column, "Respawn at Base", RESPAWN_AT_BASE, SPAWN_BUTTON);
        disabled_if(commands, base, !view.can_spawn);
    }

    let city = choice(commands, column, "Enter City", ENTER_CITY, SPAWN_BUTTON);
    commands.entity(city).insert(Shortcut(KeyCode::Digit1));
    disabled_if(commands, city, !view.can_spawn);
}

fn lobby(commands: &mut Commands, panel: Entity) {
    let corner = group(commands, panel, 828.0, 32.0, FlexDirection::Column);
    let ready = choice(commands, corner, "Ready", READY, Vec2::new(192.0, 32.0));
    commands.entity(ready).insert(Shortcut(KeyCode::KeyR));
}

fn requisition(commands: &mut Commands, panel: Entity, view: &MenuView, price: &dyn Fn(ItemKind) -> i32) {
    let tabs = group(commands, panel, 4.0, 64.0, FlexDirection::Row);
    let mut tab_list = vec![("Weapons", WEAPONS_TAB), ("Ammo", AMMO_TAB), ("Equipment", EQUIPMENT_TAB)];

    if view.mode != GameMode::Versus {
        tab_list.extend([("Vehicles", VEHICLES_TAB), ("Stocks", STOCKS_TAB)]);
    }

    if view.manager_tab {
        tab_list.push(("Manager", MANAGER_TAB));
    }

    for (label, value) in tab_list {
        choice(commands, tabs, label, value, TAB_BUTTON);
    }

    let left = group(commands, panel, 32.0, 128.0, FlexDirection::Column);
    let right = group(commands, panel, 512.0, 128.0, FlexDirection::Column);

    let items: &[(&str, ItemKind, i32)] = match view.menu {
        MenuType::RoundCorpWeapons => &WEAPONS,
        MenuType::RoundCorpAmmo => &AMMO,
        MenuType::RoundCorpEquip => &EQUIPMENT,
        _ => &[],
    };

    for &(label, kind, value) in items {
        let b = choice(commands, left, label, value, BUTTON);
        with_price(commands, b, price(kind));
    }

    match view.menu {
        MenuType::RoundCorpVehicle => {
            for (k, button) in view.buttons.iter().enumerate() {
                let b = choice(commands, if k < 4 { left } else { right }, &button.text, button.id, BUTTON);
                with_price(commands, b, button.extra);
                disabled_if(commands, b, button.extra > view.money);
            }
        }
        MenuType::RoundCorpStock => {
            for (k, n) in SHARES.into_iter().enumerate() {
                choice(commands, left, &format!("Buy {n} Share{}", if n == 1 { "" } else { "s" }), k as i32, BUTTON);
                choice(commands, right, &format!("Sell {n} Share{}", if n == 1 { "" } else { "s" }), 4 + k as i32, BUTTON);
            }
        }
        MenuType::WorldEmptyCorp => {
            for (k, button) in view.buttons.iter().enumerate() {
                choice(commands, if k < 10 { left } else { right }, &button.text, button.id, SMALL_BUTTON);
            }
        }
        _ => {}
    }

    done_button(commands, panel, 32.0, 440.0);
}

fn server_buttons(commands: &mut Commands, panel: Entity, view: &MenuView) {
    let tabs = group(commands, panel, 16.0, 64.0, FlexDirection::Row);

    for button in view.buttons.iter().filter(|b| b.id & TAB != 0) {
        choice(commands, tabs, &button.text, button.id, TAB_BUTTON);
    }

    let mut column = Entity::PLACEHOLDER;

    for (k, button) in view.buttons.iter().filter(|b| b.id & TAB == 0).enumerate() {
        if k % 8 == 0 {
            column = group(commands, panel, 16.0 + (k / 8) as f32 * 284.0, 128.0, FlexDirection::Column);
        }

        let b = choice(commands, column, &button.text, button.id, Vec2::new(252.0, 36.0));
        disabled_if(commands, b, button.id & DISABLED != 0);
    }

    done_button(commands, panel, 16.0, 480.0);
}

fn on_choice(activate: On<Activate>, choices: Query<&Choice>, done: Query<(), With<Done>>, net: Option<ResMut<NetGame>>, mut next: ResMut<NextState<GameMenu>>) {
    if let (Ok(&Choice(value)), Some(mut net)) = (choices.get(activate.entity), net) {
        net.menu_action(value as u32, 0);
    } else if done.contains(activate.entity) {
        next.set(GameMenu::Closed);
    }
}
