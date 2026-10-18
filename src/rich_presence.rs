use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use bevy::prelude::*;
use discord_sdk::{
    Discord, DiscordApp, OffsetDateTime, Subscriptions,
    activity::{ActivityBuilder, ActivityKind, PartyPrivacy},
    wheel::{UserState, Wheel},
};
use ferrosa_protocol::{GameMode, Team, clientbound::game::GameState};
use tokio::runtime::Runtime;

use crate::{
    game::{
        netgame::NetGame,
        round_events::{GameStateChanged, TeamChanged},
    }, menu::Screen, steam::Steam,
};

const SPACING: Duration = Duration::from_secs(4);

#[derive(Clone, PartialEq, Debug)]
struct Activity {
    details: String,
    state: Option<String>,
    party: Option<String>,
    since: Option<i64>,
}

#[derive(Resource)]
struct DiscordLink {
    runtime: Runtime,
    discord: Arc<Discord>,
    connected: Arc<AtomicBool>,
}

#[derive(Resource, Default)]
struct Presence {
    sent: Option<Activity>,
    last_discord_send: Option<Duration>,
    last_steam_send: Option<Duration>,
    round_start: Option<i64>,
}

pub struct RichPresencePlugin;

impl Plugin for RichPresencePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Presence>()
            .add_systems(Startup, connect)
            .add_observer(on_state_changed)
            .add_observer(|event: On<TeamChanged>, mut presence: ResMut<Presence>| {
                debug!("discord presence: team now {:?}", event.team);
                presence.sent = None;
            })
            .add_systems(Update, push_discord_presence.run_if(resource_exists::<DiscordLink>))
            .add_systems(Update, push_steam_presence)
            .add_systems(OnEnter(Screen::InGame), |mut presence: ResMut<Presence>| presence.round_start = Some(now()))
            .add_systems(OnExit(Screen::InGame), |mut presence: ResMut<Presence>| presence.round_start = None);
    }
}

fn connect(mut commands: Commands) {
    let runtime = match tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build() {
        Ok(r) => r,
        Err(e) => {
            warn!("discord presence: no runtime: {e}");
            return;
        }
    };

    let (wheel, handler) = Wheel::new(Box::new(|e| warn!("discord presence: {e}")));

    let discord = match runtime.block_on(async {
        Discord::new(DiscordApp::PlainId(1561177535243358238), Subscriptions::ACTIVITY, Box::new(handler))
    }) {
        Ok(d) => Arc::new(d),
        Err(e) => {
            warn!("discord presence: not connected: {e}");
            return;
        }
    };

    let connected = Arc::new(AtomicBool::new(false));
    let watching = connected.clone();
    let mut user = wheel.user().0;

    runtime.spawn(async move {
        loop {
            watching.store(matches!(*user.borrow(), UserState::Connected(_)), Ordering::Relaxed);

            if user.changed().await.is_err() {
                break;
            }
        }
    });

    commands.insert_resource(DiscordLink { runtime, discord, connected });
}

fn now() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}

fn on_state_changed(event: On<GameStateChanged>, mut presence: ResMut<Presence>) {
    if event.to == GameState::InGame && event.from != Some(GameState::InGame) {
        presence.round_start = Some(now());
    }

    presence.sent = None;
}

fn mode_name(mode: GameMode) -> &'static str {
    match mode {
        GameMode::World => "World",
        GameMode::Eliminator => "Eliminator",
        _ => "Round",
    }
}

fn in_game(net: &NetGame, round_start: Option<i64>) -> Activity {
    let mode = net.sync.game_type;

    let team_text = if let Some(me) = net.me() {
        match me.team {
            Team::Goldmen => "Improving Goldmen's stock value",
            Team::Monsota => "Doing Monsota's dirty work",
            Team::OXS => "Completing the mission for OXS",
            Team::Nexaco => "Nexaco's top employee",
            Team::Pentacom => "Finishing the job for Pentacom",
            Team::Prodocon => "Producing results for Prodocon",
            Team::Mission => "How did you get here...",
            Team::Spectator => "Hanging around"
        }
    } else {
        "Looking for work"
    };

    let state = match (mode, net.game_state) {
        (GameMode::World, _) => team_text.to_string(),
        (GameMode::Eliminator, GameState::InGame) => "Surviving".to_string(),
        (_, GameState::InGame) => team_text.to_string(),
        (_, GameState::Restarting) => "Waiting for the round to restart".to_string(),
        (GameMode::Eliminator, _) => "Hanging in the lobby".to_string(),
        _ => team_text.to_string(),
    };

    let players = net.players.iter().filter(|p| p.active && !p.is_bot).count();
    let plural = if players == 1 { "" } else { "s" };
    let since = (mode == GameMode::World || net.game_state == GameState::InGame).then_some(round_start).flatten();

    Activity { details: format!("In {} with {players} player{plural}.", mode_name(mode)), state: Some(state), party: Some(net.server.to_string()), since }
}

fn wanted(screen: &Screen, net: Option<&NetGame>, presence: &Presence) -> Activity {
    match (screen, net) {
        (Screen::InGame, Some(net)) => in_game(net, presence.round_start),
        _ => Activity { details: "In the menus".to_string(), state: None, party: None, since: None },
    }
}

fn push_steam_presence(screen: Res<State<Screen>>, net: Option<Res<NetGame>>, time: Res<Time<Real>>, steam: Res<Steam>, mut presence: ResMut<Presence>) {
    let activity = wanted(screen.get(), net.as_deref(), &presence);
    let elapsed = time.elapsed();

    if presence.last_steam_send.is_some_and(|t| elapsed - t < SPACING) {
        return
    }

    if let Some(ref client) = steam.client {
        let friends = client.friends();

        friends.set_rich_presence("steam_display", activity.state.as_deref());
        friends.set_rich_presence("steam_player_group", activity.party.as_deref());
        friends.set_rich_presence("gamestatus", activity.state.as_deref());
    }

    presence.last_steam_send = Some(elapsed);
}

fn push_discord_presence(time: Res<Time<Real>>, screen: Res<State<Screen>>, net: Option<Res<NetGame>>, link: Res<DiscordLink>, mut presence: ResMut<Presence>, steam: Res<Steam>) {
    if !link.connected.load(Ordering::Relaxed) {
        presence.sent = None;
        return;
    }

    let activity = wanted(screen.get(), net.as_deref(), &presence);
    let elapsed = time.elapsed();

    if presence.sent.as_ref() == Some(&activity) || presence.last_discord_send.is_some_and(|t| elapsed - t < SPACING) {
        return;
    }

    let mut builder = ActivityBuilder::new().kind(ActivityKind::Playing).details(activity.details.clone());

    if let Some(state) = &activity.state {
        builder = builder.state(state.clone());
    }

    if let Some(id) = &activity.party {
        builder = builder.party(id.clone(), None, None, PartyPrivacy::Private);
    }

    if let Some(since) = activity.since {
        builder = builder.start_timestamp(since);
    }

    let discord = link.discord.clone();

    link.runtime.spawn(async move {
        if let Err(e) = discord.update_activity(builder).await {
            warn!("discord presence not updated: {e}");
        }
    });

    presence.sent = Some(activity);
    presence.last_discord_send = Some(elapsed);
}
