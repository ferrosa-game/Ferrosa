use bevy::prelude::*;
use ferrosa_protocol::{GameMode, clientbound::game::GameState};

use super::{Stick, align, anchored, shown, spawn_screen, text};
use crate::{
    fonts::GameFonts,
    game::{GAME_FPS, escape_menu::OVERLAY_LAYER, netgame::NetGame},
};

#[derive(Component)]
pub(super) struct TimerScreen;

#[derive(Component)]
pub(super) struct RoundTimer;

pub(super) fn spawn(commands: &mut Commands, fonts: &GameFonts) {
    let top_right = spawn_screen(commands, TimerScreen, Stick::TopRight);
    commands.entity(top_right).insert(GlobalZIndex(OVERLAY_LAYER));

    let (node, shift) = anchored(align::RIGHT, Vec2::new(1020.0, 4.0));
    commands.spawn((node, shift, RoundTimer, text("", &fonts.osd_white, 24.0, Color::WHITE), ChildOf(top_right)));
}

fn clock(ticks: i32) -> String {
    let seconds = ticks.max(0) / GAME_FPS as i32;

    format!("{}:{:02}", seconds / 60, seconds % 60)
}

pub(super) fn show_timer(net: Res<NetGame>, mut screen: Single<&mut Visibility, With<TimerScreen>>, mut timer: Single<&mut Text, With<RoundTimer>>) {
    let timer_ticks = net.timer as i32;
    let shown_text = match (net.sync.game_type, net.game_state) {
        (GameMode::Round | GameMode::Eliminator, GameState::Idle | GameState::Intermission) => Some(format!("Start: {}", clock(timer_ticks))),
        (GameMode::Round | GameMode::Eliminator, GameState::InGame) => Some(clock(timer_ticks)),
        (GameMode::Round | GameMode::Eliminator, GameState::Restarting) => Some(format!("Restart: {}", clock(timer_ticks))),
        _ => None,
    };

    shown(&mut screen, shown_text.is_some());

    if let Some(s) = shown_text
        && timer.0 != s
    {
        timer.0 = s;
    }
}
