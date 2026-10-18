use bevy::prelude::*;
use ferrosa_protocol::{
    GameMode, Team,
    clientbound::game::{GameState, MenuType},
};

use super::{
    escape_menu::GameMenu,
    first_person::FirstPerson,
    hud::{Stick, align, anchored, colour, spawn_screen, text},
    mute::{Voice, VoiceIcons, voice_icon},
    netgame::NetGame,
};
use crate::{fonts::GameFonts, menu::Screen};

const TEAM_COLOURS: [[f32; 3]; 8] =
    [[0.75, 0.5625, 0.1875], [0.1875, 0.375, 0.5625], [0.25, 0.25, 0.25], [0.5625, 0.1875, 0.1875], [0.875, 0.875, 0.875], [0.125, 0.5, 0.125], [0.75, 0.75, 0.75], [0.25, 0.25, 0.25]];
const WATCHED_FALLBACK: usize = 2;
const LISTED_FALLBACK: usize = 6;
const LIST_AT: Vec2 = Vec2::new(752.0, 64.0);
const LINE: f32 = 20.0;
const NO_ONE: i32 = -1;

#[derive(Resource, Default)]
struct AdminWatching(bool);

#[derive(Component)]
struct SpectatorScreen;

#[derive(Component)]
struct WatchedLines;

#[derive(Component)]
struct WatchedName;

#[derive(Component)]
struct Humanless;

#[derive(Resource, Default, PartialEq)]
struct Listed(Vec<(String, Team, Voice)>);

pub struct SpectatingPlugin;

impl Plugin for SpectatingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AdminWatching>()
            .init_resource::<Listed>()
            .add_systems(OnEnter(Screen::InGame), spawn_spectator_hud)
            .add_systems(Update, (choose_target, show_spectator_hud).chain().run_if(in_state(Screen::InGame)).run_if(resource_exists::<NetGame>))
            .add_systems(OnExit(Screen::InGame), |mut admin: ResMut<AdminWatching>, mut listed: ResMut<Listed>| {
                admin.0 = false;
                listed.0.clear();
            });
    }
}

pub fn spectates(mode: GameMode) -> bool {
    matches!(mode, GameMode::Round | GameMode::Eliminator | GameMode::CoOp | GameMode::Versus)
}

fn team_colour(team: Team, fallback: usize) -> Color {
    let [r, g, b] = TEAM_COLOURS.get(team as usize).copied().unwrap_or(TEAM_COLOURS[fallback]);

    colour([r, g, b, 1.0])
}

fn shadowed(s: impl Into<String>, font: &Handle<Font>, size: f32, colour: Color) -> impl Bundle {
    (text(s, font, size, colour), TextShadow { offset: Vec2::splat(size / 16.0), color: Color::BLACK })
}

fn cycle(net: &mut NetGame, back: bool) {
    let step = |i: u8| if back { i.wrapping_sub(1) } else { i.wrapping_add(1) };
    let (start, mut i) = if net.spectating < 0 { (u8::MAX, 0) } else { (net.spectating as u8, step(net.spectating as u8)) };

    while i != start {
        if net.humans.get(i as usize).is_some_and(|h| h.active && h.alive) {
            net.spectating = i as i32;
            return;
        }

        i = step(i);
    }
}

fn choose_target(keys: Res<ButtonInput<KeyCode>>, menu: Res<State<GameMenu>>, fp: Res<FirstPerson>, mut net: ResMut<NetGame>, mut admin: ResMut<AdminWatching>) {
    let listening = matches!(menu.get(), GameMenu::Closed | GameMenu::PlayerMenu);
    let back = keys.pressed(KeyCode::ShiftLeft);
    let (free, next) = (listening && keys.pressed(KeyCode::Digit1), listening && keys.just_pressed(KeyCode::Space));

    if net.sync.game_type == GameMode::World {
        let Some(p) = net.me() else { return };

        if !p.is_admin {
            admin.0 = false;
            return;
        }

        if p.menu == MenuType::EnterCity && listening && keys.just_pressed(KeyCode::Digit0) {
            admin.0 = !admin.0;
        }

        if admin.0 {
            if free {
                net.spectating = NO_ONE;
            }

            if next {
                cycle(&mut net, back);
            }
        }

        return;
    }

    admin.0 = false;

    if !spectates(net.sync.game_type) || fp.active {
        return;
    }

    if free {
        net.spectating = NO_ONE;
    }

    if next {
        cycle(&mut net, back);
    }

    if net.game_state != GameState::InGame {
        net.spectating = NO_ONE;
    }
}

fn spawn_spectator_hud(mut commands: Commands, fonts: Res<GameFonts>) {
    let bottom = spawn_screen(&mut commands, SpectatorScreen, Stick::Bottom);
    let right = spawn_screen(&mut commands, SpectatorScreen, Stick::TopRight);
    let white = Color::WHITE;

    let (node, shift) = anchored(align::CENTRE_X, Vec2::new(512.0, 512.0));
    commands.spawn((node, shift, shadowed("Spectating", &fonts.rockwell, 16.0, white), ChildOf(bottom)));

    let lines = commands.spawn((Node { position_type: PositionType::Absolute, ..default() }, WatchedLines, Pickable::IGNORE, ChildOf(bottom))).id();
    let (node, shift) = anchored(align::CENTRE_X, Vec2::new(512.0, 528.0));
    commands.spawn((node, shift, WatchedName, shadowed("", &fonts.rockwell, 20.0, white), ChildOf(lines)));
    let (node, shift) = anchored(align::CENTRE_X, Vec2::new(512.0, 548.0));
    commands.spawn((node, shift, shadowed("Press Space to switch player", &fonts.rockwell, 16.0, white), ChildOf(lines)));
    let (node, shift) = anchored(align::CENTRE_X, Vec2::new(512.0, 564.0));
    commands.spawn((node, shift, shadowed("Press 1 for freecam", &fonts.rockwell, 16.0, white), ChildOf(lines)));

    let list = Node { position_type: PositionType::Absolute, left: px(LIST_AT.x), top: px(LIST_AT.y), flex_direction: FlexDirection::Column, ..default() };
    commands.spawn((list, Humanless, Pickable::IGNORE, ChildOf(right)));
}

fn watched(net: &NetGame) -> Option<(String, Team)> {
    let human = net.humans.get(usize::try_from(net.spectating).ok()?)?;
    let player = net.players.get(usize::try_from(human.player).ok()?)?;

    Some((player.name.clone(), player.team))
}

#[allow(clippy::too_many_arguments)]
fn show_spectator_hud(
    mut commands: Commands,
    net: Res<NetGame>,
    fp: Res<FirstPerson>,
    fonts: Res<GameFonts>,
    icons: Res<VoiceIcons>,
    mut listed: ResMut<Listed>,
    mut screens: Query<&mut Visibility, (With<SpectatorScreen>, Without<WatchedLines>)>,
    mut lines: Single<&mut Visibility, With<WatchedLines>>,
    name: Single<(&mut Text, &mut TextColor), With<WatchedName>>,
    list: Single<Entity, With<Humanless>>,
) {
    let humanless = net.me().is_some_and(|p| p.human < 0);
    let shown = spectates(net.sync.game_type) && net.game_state == GameState::InGame && !fp.active && humanless;

    for mut v in &mut screens {
        v.set_if_neq(if shown { Visibility::Inherited } else { Visibility::Hidden });
    }

    let target = watched(&net);

    lines.set_if_neq(if target.is_some() { Visibility::Inherited } else { Visibility::Hidden });

    if let Some((who, team)) = target {
        let (mut t, mut c) = name.into_inner();

        if t.0 != who {
            t.0 = who;
        }

        c.set_if_neq(TextColor(team_colour(team, WATCHED_FALLBACK)));
    }

    let now = Listed(net.players.iter().filter(|p| p.active && !p.is_bot && p.human < 0).map(|p| (p.name.clone(), p.team, Voice::of(p))).collect());

    if *listed == now {
        return;
    }

    commands.entity(*list).despawn_children();

    for (who, team, voice) in &now.0 {
        let row = commands.spawn((Node { height: px(LINE), align_items: AlignItems::Center, ..default() }, Pickable::IGNORE, ChildOf(*list))).id();

        voice_icon(&mut commands, row, &icons, *voice, LINE, None);
        commands.spawn((shadowed(who.clone(), &fonts.rockwell, LINE, team_colour(*team, LISTED_FALLBACK)), ChildOf(row)));
    }

    *listed = now;
}
