use bevy::prelude::*;
use ferrosa_protocol::{Team, clientbound::game::GameState};

use super::netgame::NetGame;

#[derive(Event, Clone, Copy, Debug)]
pub struct GameStateChanged {
    pub from: Option<GameState>,
    pub to: GameState,
}

#[derive(Event, Clone, Copy, Debug)]
pub struct TeamChanged {
    pub team: Option<Team>,
}

pub fn watch(commands: &mut Commands, net: &mut NetGame) {
    let state = net.game_state;
    let team = net.me().map(|p| p.team);

    if net.seen_state != Some(state) {
        commands.trigger(GameStateChanged { from: net.seen_state, to: state });
        net.seen_state = Some(state);
    }

    if net.seen_team != team {
        commands.trigger(TeamChanged { team });
        net.seen_team = team;
    }
}
