use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    time::Duration,
};

use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task, futures::check_ready},
};
use ferrosa_protocol::{
    codec::{Reader, WireRead},
    frame_packet,
    masterserver::{
        discovery::resolve_address,
        sign_in::{AuthConfirm, AuthConfirmReply, SteamAuth, SteamAuthReply},
    },
};

use crate::{config::Config, names::phone_number, net::ClientSocket, steam::Steam};

const MASTERSERVER_SITE: &str = "www.crypticsea.com";
const SERVERINFO_TIMEOUT: Duration = Duration::from_secs(3);
pub const FALLBACK: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(66, 226, 72, 227), 0x6bc8);
const DEFAULT_NAME: &str = "Player";

pub fn get_masterserver_info() -> Option<SocketAddrV4> {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().ok()?;
    match runtime.block_on(async { tokio::time::timeout(SERVERINFO_TIMEOUT, resolve_address(MASTERSERVER_SITE, None)).await }) {
        Ok(Ok(SocketAddr::V4(addr))) => Some(addr),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthState {
    Ticket,
    Confirm,
    Done,
    Failed,
}

#[derive(Resource)]
pub struct MasterAuth {
    pub server: SocketAddrV4,
    pub state: AuthState,
    pub game_auth: u32,
    pub master_auth: u32,
    pub phone: u32,
    last_send: u32,
}

impl MasterAuth {
    pub fn new(server: SocketAddrV4, now: u32) -> Self {
        Self { server, state: AuthState::Ticket, game_auth: 0, master_auth: 0, phone: 0, last_send: now.wrapping_sub(1000) }
    }

    pub fn update(&mut self, socket: &mut ClientSocket, steam: &mut Steam, name: &str, now: u32) {
        if !matches!(self.state, AuthState::Ticket | AuthState::Confirm) {
            return;
        }

        if self.state == AuthState::Ticket && self.master_auth != 0 {
            self.state = AuthState::Confirm;
        }

        if now.wrapping_sub(self.last_send) > 1000 {
            self.last_send = now;
            if self.state == AuthState::Ticket
                && let Some(ticket) = steam.ticket.as_ref().filter(|t| t.len() <= 0x3ff)
            {
                let p = SteamAuth { name: name.chars().take(0x20).collect(), ticket: ticket.clone() };
                socket.send(&frame_packet(p), self.server);
            }
            if self.state == AuthState::Confirm {
                socket.send(&frame_packet(AuthConfirm { game_auth: self.game_auth, master_auth: self.master_auth }), self.server);
            }
        }

        while let Some(packet) = socket.recv() {
            let mut r = Reader::new(&packet.data[4..]);

            match r.u8() {
                Ok(0x48) => {
                    let Ok(reply) = SteamAuthReply::read(&mut r) else { continue };
                    self.game_auth = reply.game_auth;
                    self.master_auth = reply.master_auth;

                    if let Some(phone) = reply.phone_number {
                        self.phone = phone;
                        self.state = AuthState::Confirm;
                    } else {
                        self.state = AuthState::Failed;
                    }

                    self.last_send = now.wrapping_sub(1000);
                }
                Ok(0x49) => {
                    let Ok(reply) = AuthConfirmReply::read(&mut r) else { continue };

                    if reply.accepted {
                        self.phone = reply.phone_number.unwrap_or(self.phone);
                        self.state = AuthState::Done;
                    } else {
                        self.master_auth = 0;
                        self.state = AuthState::Ticket;
                    }
                }
                _ => {}
            }
        }

        if steam.ok() && !steam.requested_ticket {
            steam.request_encrypted_app_ticket();
        }
    }
}

#[derive(Resource)]
pub struct PlayerName(pub String);

#[derive(Resource)]
struct ServerInfoFetch(Task<Option<SocketAddrV4>>);

pub struct MasterServerPlugin;

impl Plugin for MasterServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, start).add_systems(Update, (run_steam_callbacks, finish_server_info, sign_in).chain());
    }
}

fn start(mut commands: Commands, config: Option<Res<Config>>) {
    let mut steam = Steam::init();
    let name = if steam.user_id().is_some() { steam.persona_name() } else { None }.or_else(|| config.map(|c| c.name.clone())).filter(|n| !n.is_empty());

    commands.insert_resource(PlayerName(name.map_or_else(|| DEFAULT_NAME.to_string(), |n| n.chars().take(0x1f).collect())));
    commands.insert_resource(steam);
    commands.insert_resource(ClientSocket::open());
    commands.insert_resource(ServerInfoFetch(IoTaskPool::get().spawn(async { get_masterserver_info() })));
    // TODO: the Steam friends list (steam_get_friend_count, steam_get_friend_name).
}

fn run_steam_callbacks(mut steam: ResMut<Steam>) {
    steam.run_callbacks();
}

fn finish_server_info(mut commands: Commands, fetch: Option<ResMut<ServerInfoFetch>>, time: Res<Time<Real>>) {
    let Some(mut fetch) = fetch else { return };
    let Some(info) = check_ready(&mut fetch.0) else { return };

    commands.remove_resource::<ServerInfoFetch>();
    commands.insert_resource(MasterAuth::new(info.unwrap_or(FALLBACK), time.elapsed().as_millis() as u32));
}

fn sign_in(master: Option<ResMut<MasterAuth>>, mut socket: ResMut<ClientSocket>, mut steam: ResMut<Steam>, name: Res<PlayerName>, time: Res<Time<Real>>) {
    if let Some(mut master) = master {
        master.update(&mut socket, &mut steam, &name.0, time.elapsed().as_millis() as u32);
    }
}

pub fn auth_status(master: Option<&MasterAuth>, steam: &Steam) -> String {
    match master.map(|m| (m.state, m.phone)) {
        None => "Connecting...".to_string(),
        Some((AuthState::Confirm, _)) => "Authenticating...".to_string(),
        Some((AuthState::Done, phone)) => format!("Phone Number: {}", phone_number(phone)),
        Some((AuthState::Failed, _)) => "Auth Failed".to_string(),

        _ if !steam.ok() => "Steam Init Failed".to_string(),
        _ if steam.ticket.is_none() => "Steam Ticket...".to_string(),
        _ => "Steam Auth...".to_string(),
    }
}