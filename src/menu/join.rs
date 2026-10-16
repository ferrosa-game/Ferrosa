use bevy::{prelude::*, ui_widgets::{Activate, ValueChange}};
use ferrosa_protocol::{
    GameMode,
    clientbound::{initial_sync::InitialSync, kick::KickClient},
    codec::{Reader, WireRead},
    frame_packet,
    masterserver::ticket::{Ticket, TicketRequest},
    serverbound::join_request::{AvatarInfo, JoinRequest},
};

use crate::{
    game::netgame::NetGame,
    masterserver::{MasterAuth, PlayerName},
    menu::{Screen, character::PlayerLook, gap, page, servers::JoinTarget, text},
    net::ClientSocket,
    ui::{self, Shortcut},
};

const RESEND_EVERY: u32 = 200;
const PASSWORD_LENGTH: usize = 0x18;
const ITEM: Vec2 = Vec2::new(240.0, 32.0);
const HEADING: Color = Color::srgb(0.75, 0.75, 0.75);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinState {
    Password,
    Ticket,
    Joining,
    Joined,
    Kicked,
}

#[derive(Resource)]
pub struct Join {
    pub state: JoinState,
    ticket: u32,
    password: String,
    kick: String,
    last_send: Option<u32>,
}

impl Join {
    pub fn start(password: bool) -> Self {
        Self { state: if password { JoinState::Password } else { JoinState::Ticket }, ticket: 0, password: String::new(), kick: String::new(), last_send: None }
    }
}

#[derive(Resource, Clone, Debug)]
pub struct ServerSync {
    pub round: u32,
    pub game_type: GameMode,
    pub weekly: bool,
    pub weekday: u32,
    pub map: String,
    pub sun_angle: f32,
    pub sun_tilt: f32,
    pub move_delay: u32,
}

fn angle16(v: u16) -> f32 {
    let sign = 0x8000u32;
    let a = ((((sign - 1) & v as u32) as f32) as f64 / ((sign as f32) as f64 / std::f64::consts::TAU)) as f32;
    if v as u32 & sign == sign { -a } else { a }
}

impl From<InitialSync> for ServerSync {
    fn from(s: InitialSync) -> Self {
        Self {
            round: s.round_number,
            game_type: s.gamemode,
            weekly: s.weekly_enabled,
            weekday: s.weekday as u32,
            map: s.map_name,
            sun_angle: angle16(s.sun_angle),
            sun_tilt: angle16(s.sun_axial_tilt),
            move_delay: s.versus_movedelay.unwrap_or(0) as u32,
        }
    }
}

#[derive(Component)]
struct PasswordPrompt;

#[derive(Component)]
struct JoinStatus;

pub(super) struct JoinPlugin;

impl Plugin for JoinPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::Connecting), connecting)
            .add_systems(Update, (join_server, show_join).chain().run_if(in_state(Screen::Connecting)));
    }
}

fn connecting(mut commands: Commands, target: Option<Res<JoinTarget>>) {
    let p = page(&mut commands, Screen::Connecting);

    let name = target.as_ref().map_or("", |t| t.name.as_str());
    text(&mut commands, p, &format!("Joining {name}"), 32.0, Color::WHITE);

    commands.spawn((Text::default(), TextFont::from_font_size(20.0), TextColor(HEADING), JoinStatus, Node { margin: UiRect::vertical(px(16)), ..default() }, ChildOf(p)));

    let prompt = commands.spawn((Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: px(8), ..default() }, PasswordPrompt, ChildOf(p))).id();
    let password = ui::text::password_input(&mut commands, prompt, "Password", "", PASSWORD_LENGTH, ITEM.x);
    commands.entity(password).observe(|v: On<ValueChange<String>>, mut join: ResMut<Join>| {
        join.password.clone_from(&v.value);
        if v.is_final && join.state == JoinState::Password {
            join.state = JoinState::Ticket;
        }
    });

    let connect = ui::button(&mut commands, prompt, "Connect", ITEM);
    commands.entity(connect).observe(|_: On<Activate>, mut join: ResMut<Join>| {
        if join.state == JoinState::Password {
            join.state = JoinState::Ticket;
        }
    });

    gap(&mut commands, p, 24.0);

    let cancel = ui::button(&mut commands, p, "Cancel", Vec2::new(320.0, 44.0));
    commands.entity(cancel).insert(Shortcut(KeyCode::Escape)).observe(|_: On<Activate>, mut next: ResMut<NextState<Screen>>| next.set(Screen::ServerList));
}

fn join_server(
    mut commands: Commands,
    time: Res<Time<Real>>,
    target: Option<Res<JoinTarget>>,
    join: Option<ResMut<Join>>,
    master: Option<Res<MasterAuth>>,
    name: Option<Res<PlayerName>>,
    look: Res<PlayerLook>,
    mut socket: ResMut<ClientSocket>,
    mut next: ResMut<NextState<Screen>>,
) {
    let (Some(target), Some(mut join), Some(master)) = (target, join, master) else { return };

    if !matches!(join.state, JoinState::Ticket | JoinState::Joining) {
        return;
    }

    let now = time.elapsed().as_millis() as u32;

    if join.last_send.is_none_or(|t| now.wrapping_sub(t) > RESEND_EVERY) {
        join.last_send = Some(now);

        if join.state == JoinState::Ticket {
            let p = TicketRequest { game_auth: master.game_auth, master_auth: master.master_auth, server: target.advertised };

            socket.send(&frame_packet(p), master.server);
        } else {
            let look_byte = |v: i32| v.clamp(0, 0xff) as u8;

            let p = JoinRequest {
                version: 38_u8,
                account_id: master.game_auth,
                auth_ticket: join.ticket,
                player_name: name.map(|n| n.0.clone()).unwrap_or_default(),
                avatar_info: AvatarInfo {
                    gender: look_byte(look.gender),
                    head: look_byte(look.head),
                    skin_color: look_byte(look.skin),
                    hair_color: look_byte(look.hair_colour),
                    hair: look_byte(look.hair),
                    eye_color: look_byte(look.eyes),
                },
                password: join.password.clone(),
                phone_number: master.phone,
                protocol_version: 71,
            };

            socket.send(&frame_packet(p), target.addr);
        }
    }

    while let Some(packet) = socket.recv() {
        let mut r = Reader::new(&packet.data[4..]);

        match r.u8() {
            Ok(0x4b) => {
                let Ok(t) = Ticket::read(&mut r) else { continue };
                join.ticket = t.ticket;
                join.state = JoinState::Joining;
                join.last_send = None;
            }
            Ok(6) => {
                let Ok(sync) = InitialSync::read(&mut r) else { continue };
                let sync = ServerSync::from(sync);
                info!("joined {} on {} ({:?})", target.name, sync.map, sync.game_type);
                commands.insert_resource(NetGame::new(target.addr, sync.clone()));
                commands.insert_resource(sync);
                join.state = JoinState::Joined;
                next.set(Screen::Loading);
                break;
            }
            Ok(3) => {
                let Ok(kick) = KickClient::read(&mut r) else { continue };
                join.kick = kick.reason;
                join.state = JoinState::Kicked;
            }
            _ => {}
        }
    }
}

fn show_join(
    join: Option<Res<Join>>,
    sync: Option<Res<ServerSync>>,
    mut status: Query<&mut Text, With<JoinStatus>>,
    mut prompts: Query<&mut Node, With<PasswordPrompt>>,
) {
    let Some(join) = join else { return };

    let want = match join.state {
        JoinState::Password => "This server has a password".to_string(),
        JoinState::Ticket => "Asking the masterserver for a ticket...".to_string(),
        JoinState::Joining => "Connecting...".to_string(),
        JoinState::Joined => sync.map_or_else(|| "Joined".to_string(), |s| format!("Joined: {}", s.map)),
        JoinState::Kicked => join.kick.clone(),
    };

    for mut text in &mut status {
        if text.0 != want {
            text.0.clone_from(&want);
        }
    }

    let display = if join.state == JoinState::Password { Display::Flex } else { Display::None };
    for mut node in &mut prompts {
        if node.display != display {
            node.display = display;
        }
    }
}
