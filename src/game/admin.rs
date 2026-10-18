use bevy::{prelude::*, ui_widgets::{Activate, ValueChange}};
use ferrosa_protocol::serverbound::game::actions::{AdminAction, GameAction};

use super::{
    GAME_SCREEN,
    escape_menu::{GameMenu, MENU_LAYER},
    netgame::NetGame,
};
use crate::{
    menu::Screen,
    names::phone_number,
    ui::{self, Shortcut},
};

const SHADE: Color = Color::srgba(0.0, 0.0, 0.0, 0.5);
const BACK_AT: Vec2 = Vec2::new(16.0, 16.0);
const BACK: Vec2 = Vec2::new(240.0, 32.0);
const LIST_AT: Vec2 = Vec2::new(128.0, 64.0);
const ROW: Vec2 = Vec2::new(192.0, 28.0);
const FIELD_WIDTH: f32 = 120.0;
const BAN: u8 = 4;
const SET_RATING: u8 = 5;
const MAX_TIME: i32 = 100_000_000;
const MAX_RATING: i32 = 10_000;

#[derive(Resource, Default)]
struct AdminPage {
    chosen: Option<usize>,
    time: i32,
    rating: i32,
    built: Option<(Option<usize>, Vec<(usize, String, i32)>)>,
}

#[derive(Component, Clone, Copy)]
enum AdminButton {
    Back,
    Row(usize),
    Ban,
    SetRating,
}

#[derive(Component)]
struct AdminPanel;

pub struct AdminPlugin;

impl Plugin for AdminPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AdminPage>()
            .add_systems(OnEnter(GameMenu::Admin), |mut page: ResMut<AdminPage>| {
                page.chosen = None;
                page.built = None;
            })
            .add_systems(Update, build_page.run_if(in_state(GameMenu::Admin)).run_if(resource_exists::<NetGame>))
            .add_observer(on_button);
    }
}

fn at(commands: &mut Commands, parent: Entity, pos: Vec2, direction: FlexDirection) -> Entity {
    let node = Node { position_type: PositionType::Absolute, left: px(pos.x), top: px(pos.y), flex_direction: direction, row_gap: px(4), column_gap: px(8), align_items: AlignItems::Center, ..default() };

    commands.spawn((node, Pickable::IGNORE, ChildOf(parent))).id()
}

fn admin_button(commands: &mut Commands, parent: Entity, label: &str, size: Vec2, action: AdminButton) -> Entity {
    let b = ui::button(commands, parent, label, size);
    commands.entity(b).insert(action);
    b
}

fn build_page(mut commands: Commands, net: Res<NetGame>, mut page: ResMut<AdminPage>, panels: Query<Entity, With<AdminPanel>>) {
    let rows: Vec<(usize, String, i32)> = net.admin_list.iter().enumerate().filter(|(_, r)| r.filled).map(|(k, r)| (k, r.name.clone(), r.phone_number)).collect();
    let wanted = Some((page.chosen, rows.clone()));

    if page.built == wanted {
        return;
    }

    panels.iter().for_each(|e| commands.entity(e).despawn());

    let full = Node { width: percent(100), height: percent(100), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() };
    let panel = commands.spawn((full, BackgroundColor(SHADE), GlobalZIndex(MENU_LAYER), AdminPanel, DespawnOnExit(GameMenu::Admin), DespawnOnExit(Screen::InGame))).id();
    let screen = commands.spawn((Node { width: px(GAME_SCREEN.x), height: px(GAME_SCREEN.y), ..default() }, Pickable::IGNORE, ChildOf(panel))).id();

    let corner = at(&mut commands, screen, BACK_AT, FlexDirection::Column);
    let back = admin_button(&mut commands, corner, "Back", BACK, AdminButton::Back);
    commands.entity(back).insert(Shortcut(KeyCode::Escape));

    match page.chosen.and_then(|k| net.admin_list.get(k)).filter(|r| r.filled) {
        None => {
            let list = at(&mut commands, screen, LIST_AT, FlexDirection::Column);

            for (k, name, number) in &rows {
                admin_button(&mut commands, list, &format!("{name} {}", phone_number(*number as u32)), ROW, AdminButton::Row(*k));
            }
        }
        Some(row) => {
            let ban = at(&mut commands, screen, Vec2::new(32.0, 128.0), FlexDirection::Row);
            let time = ui::number_input(&mut commands, ban, "Time", page.time, (0, MAX_TIME), FIELD_WIDTH);
            commands.entity(time).observe(|v: On<ValueChange<i32>>, mut page: ResMut<AdminPage>| page.time = v.value);
            admin_button(&mut commands, ban, &format!("Ban {}", row.name), ROW, AdminButton::Ban);

            let rate = at(&mut commands, screen, Vec2::new(32.0, 176.0), FlexDirection::Row);
            let rating = ui::number_input(&mut commands, rate, "Rating", page.rating, (0, MAX_RATING), FIELD_WIDTH);
            commands.entity(rating).observe(|v: On<ValueChange<i32>>, mut page: ResMut<AdminPage>| page.rating = v.value);
            admin_button(&mut commands, rate, &format!("Set Rating {}", row.name), ROW, AdminButton::SetRating);
        }
    }

    page.built = wanted;
}

fn on_button(activate: On<Activate>, buttons: Query<&AdminButton>, net: Option<ResMut<NetGame>>, mut page: ResMut<AdminPage>, mut menu: ResMut<NextState<GameMenu>>) {
    let Ok(&button) = buttons.get(activate.entity) else { return };
    let Some(mut net) = net else { return };
    let account = page.chosen.and_then(|k| net.admin_list.get(k)).map(|r| r.account);
    let mut send = |kind: u8, value: i32| {
        if let (Some(account), Some(me)) = (account, net.me_mut()) {
            me.queue(GameAction::Admin(AdminAction { kind, a: account as u32, b: value as u32 }));
        }
    };

    match button {
        AdminButton::Back if page.chosen.is_some() => page.chosen = None,
        AdminButton::Back => menu.set(GameMenu::Open),
        AdminButton::Row(k) => page.chosen = Some(k),
        AdminButton::Ban => {
            send(BAN, page.time);
            menu.set(GameMenu::Open);
        }
        AdminButton::SetRating => {
            send(SET_RATING, page.rating);
            menu.set(GameMenu::Open);
        }
    }
}
