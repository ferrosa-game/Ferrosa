use bevy::prelude::*;
use ferrosa_protocol::{
    GameMode, Team,
    clientbound::game::{GameState, MenuType},
};

use super::{
    escape_menu::{GameMenu, ROSTER_LAYER},
    netgame::{NetGame, Player},
    player_menu::Choice,
};
use crate::{
    fonts::GameFonts,
    menu::{MENU_BACKGROUND, Screen},
    ui::{self, Shortcut},
};

const COLUMN: f32 = 248.0;
const WIDE_COLUMN: f32 = 340.0;
const COLUMNS_AT: Vec2 = Vec2::new(8.0, 80.0);
const KEY: KeyCode = KeyCode::KeyH;
const TEAM_COLOURS: [[f32; 3]; 3] = [[0.75, 0.5625, 0.1875], [0.1875, 0.375, 0.5625], [0.25, 0.25, 0.25]];
const WATCHING: [f32; 3] = [1.0; 3];
const ROUND_CORPORATIONS: [Team; 3] = [Team::Goldmen, Team::Monsota, Team::OXS];
const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const LAST_DAY: u32 = 5;
const OBSERVER: i32 = 4;
const JOIN_KEYS: [KeyCode; 3] = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3];
const BUTTON_HEIGHT: f32 = 28.0;

#[derive(Clone, PartialEq)]
enum Item {
    Text { text: String, size: f32, colour: [f32; 3] },
    Button { label: String, choice: i32, key: KeyCode },
}

#[derive(Clone, PartialEq, Default)]
struct View {
    day: Option<String>,
    winner: Option<(String, [f32; 3])>,
    columns: Vec<Vec<Item>>,
    backed: bool,
}

#[derive(Component)]
struct RosterPanel;

#[derive(Resource, Default)]
struct Shown(Option<View>);

pub struct RosterPlugin;

impl Plugin for RosterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Shown>()
            .add_systems(Update, show_roster.run_if(in_state(Screen::InGame)))
            .add_systems(OnExit(Screen::InGame), |mut shown: ResMut<Shown>| shown.0 = None);
    }
}

fn worth(p: &Player, prices: &[i32; 6]) -> i32 {
    match Team::CORPORATIONS.iter().position(|&t| t == p.team) {
        Some(t) => (prices[t] as f32 / 10.0 * p.stocks as f32 + p.money as f32) as i32,
        None => p.money,
    }
}

fn phone_number(n: i32) -> String {
    let n = n as u32;
    format!("{:03}-{:04}", n / 10000, n % 10000)
}

fn shown(net: &NetGame, held: bool) -> bool {
    let mode = net.sync.game_type;
    let peeking = matches!(mode, GameMode::Round | GameMode::World) && held;

    !matches!(mode, GameMode::Eliminator | GameMode::Versus) && (net.game_state == GameState::Restarting || in_lobby(net) || peeking)
}

fn in_lobby(net: &NetGame) -> bool {
    net.game_state == GameState::Intermission && net.me().is_some_and(|p| p.menu == MenuType::Lobby)
}

fn text(text: String, size: f32, colour: [f32; 3]) -> Item {
    Item::Text { text, size, colour }
}

// TODO: each player's speaker (talking, muted, and a click to mute them) once voice is played.
fn roster(net: &NetGame) -> View {
    let (state, prices) = (net.game_state, &net.share_prices);
    let me = net.me();
    let joining = in_lobby(net) && !matches!(net.sync.game_type, GameMode::Racing | GameMode::Eliminator);
    let watching = me.is_some_and(|p| p.team == Team::Spectator);

    let day = net.sync.weekly.then(|| DAYS.get(net.sync.weekday as usize).map(|d| d.to_string())).flatten();
    let winner = (net.sync.weekly && state == GameState::Restarting && net.sync.weekday == LAST_DAY).then(|| {
        let best = (0..3).fold(0, |best, t| if prices[t] > prices[best] { t } else { best });
        (format!("{} Wins!", ["Goldmen", "Monsota", "OXS"][best]), TEAM_COLOURS[best])
    });

    let columns = (0..4)
        .map(|column| {
            let team = ROUND_CORPORATIONS.get(column).copied();
            let colour = match column {
                2 => TEAM_COLOURS[2].map(|c| c * 1.5),
                _ => TEAM_COLOURS.get(column).copied().unwrap_or(WATCHING),
            };
            let mut items = Vec::new();

            match team {
                Some(_) if net.sync.game_type == GameMode::Round => {
                    items.push(text(format!("{} (${})", Team::CORPORATION_NAMES[column], prices[column] / 10), 20.0, colour));

                    if state == GameState::Restarting {
                        items.push(text(format!("Bonus: ${}", net.corp_round[column].0), 17.5, colour));
                    }
                }
                Some(_) => items.push(text(Team::CORPORATION_NAMES[column].to_string(), 20.0, colour)),
                None => items.push(text("Spectators".to_string(), 20.0, colour)),
            }

            match team {
                Some(_) if joining && watching => {
                    items.push(Item::Button { label: format!("Join {}", ["Goldmen", "Monsota", "OXS"][column]), choice: column as i32 + 1, key: JOIN_KEYS[column] });
                }
                None if joining && !watching => items.push(Item::Button { label: "Observer".to_string(), choice: OBSERVER, key: KeyCode::Digit4 }),
                _ => {}
            }

            let mut players: Vec<&Player> = net.players.iter().filter(|p| p.active && !p.is_bot && team.map_or(p.team == Team::Spectator, |t| p.team == t)).collect();
            players.sort_by_key(|p| std::cmp::Reverse(worth(p, prices)));

            for p in players {
                let dim = if state == GameState::Intermission && !p.is_ready { 0.5 } else { 1.0 };
                let shade = colour.map(|c| c * dim);

                items.push(text(p.name.clone(), 17.5, shade));
                items.push(text(format!("{}  ${}", phone_number(p.phone_number), worth(p, prices)), 12.0, shade));
            }

            items
        })
        .collect();

    View { day, winner, columns, backed: state == GameState::Restarting }
}

fn show_roster(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    menu: Option<Res<State<GameMenu>>>,
    net: Option<Res<NetGame>>,
    fonts: Res<GameFonts>,
    mut last: ResMut<Shown>,
    panels: Query<Entity, With<RosterPanel>>,
) {
    let typing = menu.is_some_and(|m| *m.get() == GameMenu::Chat);
    let view = net.filter(|n| shown(n, keys.pressed(KEY) && !typing)).map(|n| roster(&n));

    if view == last.0 {
        return;
    }

    panels.iter().for_each(|e| commands.entity(e).despawn());

    if let Some(view) = &view {
        spawn_roster(&mut commands, view, &fonts.rockwell);
    }

    last.0 = view;
}

fn spawn_text(commands: &mut Commands, parent: Entity, s: &str, font: &Handle<Font>, size: f32, [r, g, b]: [f32; 3], node: Node) {
    commands.spawn((Text::new(s), TextFont::from_font_size(size).with_font(font.clone()), TextColor(Color::srgb(r, g, b)), node, Pickable::IGNORE, ChildOf(parent)));
}

fn spawn_roster(commands: &mut Commands, view: &View, font: &Handle<Font>) {
    let full = Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() };
    let background = BackgroundColor(if view.backed { MENU_BACKGROUND } else { Color::NONE });
    let panel = commands.spawn((full, background, Pickable::IGNORE, GlobalZIndex(ROSTER_LAYER), RosterPanel, DespawnOnExit(Screen::InGame))).id();

    if let Some(day) = &view.day {
        spawn_text(commands, panel, day, font, 20.0, WATCHING, Node { position_type: PositionType::Absolute, left: px(8), top: px(16), ..default() });
    }

    if let Some((winner, colour)) = &view.winner {
        let across = Node { position_type: PositionType::Absolute, top: px(16), width: percent(100), justify_content: JustifyContent::Center, ..default() };
        let row = commands.spawn((across, Pickable::IGNORE, ChildOf(panel))).id();
        spawn_text(commands, row, winner, font, 30.0, *colour, Node::default());
    }

    let row = Node { position_type: PositionType::Absolute, left: px(COLUMNS_AT.x), top: px(COLUMNS_AT.y), flex_direction: FlexDirection::Row, ..default() };
    let corporations = commands.spawn((row, Pickable::IGNORE, ChildOf(panel))).id();
    let (spectators, corporation_columns) = view.columns.split_last().map_or((None, &view.columns[..]), |(last, rest)| (Some(last), rest));

    for (k, items) in corporation_columns.iter().enumerate() {
        let width = if k == 2 { WIDE_COLUMN } else { COLUMN };
        let node = Node { width: px(width), flex_direction: FlexDirection::Column, align_items: AlignItems::FlexStart, row_gap: px(2), ..default() };
        let column = commands.spawn((node, Pickable::IGNORE, ChildOf(corporations))).id();

        spawn_items(commands, column, items, width, font);
    }

    if let Some(items) = spectators {
        let node = Node {
            position_type: PositionType::Absolute,
            right: px(COLUMNS_AT.x),
            top: px(COLUMNS_AT.y),
            width: px(COLUMN),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexEnd,
            row_gap: px(2),
            ..default()
        };
        let column = commands.spawn((node, Pickable::IGNORE, ChildOf(panel))).id();

        spawn_items(commands, column, items, COLUMN, font);
    }
}

fn spawn_items(commands: &mut Commands, column: Entity, items: &[Item], width: f32, font: &Handle<Font>) {
    for item in items {
        match item {
            Item::Text { text, size, colour } => spawn_text(commands, column, text, font, *size, *colour, Node::default()),
            Item::Button { label, choice, key } => {
                let spacer = commands.spawn((Node { margin: UiRect::vertical(px(6)), ..default() }, ChildOf(column))).id();
                let b = ui::button(commands, spacer, label, Vec2::new(width - 16.0, BUTTON_HEIGHT));
                commands.entity(b).insert((Choice(*choice), Shortcut(*key)));
            }
        }
    }
}
