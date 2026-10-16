use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use bevy::{input_focus::tab_navigation::TabIndex, picking::hover::Hovered, prelude::*, ui::InteractionDisabled, ui_widgets::{Activate, Button, ScrollArea}};
use ferrosa_protocol::{
    clientbound::server_info::ServerInfo,
    codec::{Reader, WireRead},
    frame_packet,
    masterserver::sign_in::{ServerList as ServerListPacket, ServerListRequest},
    serverbound::info_request::InfoRequest,
};

use crate::{
    menu::join::Join,
    masterserver::{AuthState, MasterAuth},
    menu::{Screen, back_button, page, text},
    net::ClientSocket,
    ui,
};

const MAX_SERVERS: usize = 0x2000;
const SEND_EVERY: u32 = 10;
const LIST: Vec2 = Vec2::new(960.0, 420.0);
const COLUMNS: [(&str, f32); 5] = [("Ping", 80.0), ("Players", 90.0), ("Game Type", 120.0), ("Version", 110.0), ("Password", 100.0)];
const ROW_HEIGHT: f32 = 28.0;
const ROW_TEXT: f32 = 16.0;
const HEADING: Color = Color::srgb(0.75, 0.75, 0.75);
const OTHER_VERSION: Color = Color::srgb(0.45, 0.45, 0.45);

#[derive(Clone, Debug)]
pub struct ServerEntry {
    pub version: i32,
    pub sub_version: i32,
    pub name: String,
    pub id: u32,
    pub addr: SocketAddrV4,
    pub advertised: SocketAddrV4,
    pub players: i32,
    pub max_players: i32,
    pub ping: u32,
    pub game_type: i32,
    pub password: bool,
}

fn unspecified() -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)
}

impl ServerEntry {
    fn new(addr: SocketAddrV4) -> Self {
        Self {
            version: 0,
            sub_version: 0,
            name: String::new(),
            id: 0,
            addr,
            advertised: unspecified(),
            players: 0,
            max_players: 0,
            ping: 0,
            game_type: 0,
            password: false,
        }
    }

    fn cells(&self) -> [String; 6] {
        let mode = match self.game_type {
            5 => "Eliminator",
            3 => "Round",
            4 => "World",
            7 => "Versus",
            _ => "",
        };
        [
            self.name.clone(),
            self.ping.to_string(),
            format!("{}/{}", self.players, self.max_players),
            mode.to_string(),
            format!("Alpha {}{}", self.version, char::from_u32((self.sub_version + 0x61) as u32).unwrap_or('?')),
            if self.password { "Yes" } else { "" }.to_string(),
        ]
    }

    fn joinable(&self) -> bool {
        self.version == 38_i32
    }
}

#[derive(Resource, Clone, Debug)]
pub struct JoinTarget {
    pub name: String,
    pub addr: SocketAddrV4,
    pub advertised: SocketAddrV4,
    pub id: u32,
    pub password: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ListRequest {
    Idle,
    Asking,
    Received,
}

#[derive(Resource)]
pub struct ServerBrowser {
    pub entries: Vec<ServerEntry>,
    addresses: Vec<SocketAddrV4>,
    total: usize,
    ping_index: usize,
    last_send: u32,
    request: ListRequest,
}

impl ServerBrowser {
    fn new(now: u32) -> Self {
        Self { entries: Vec::new(), addresses: vec![unspecified(); MAX_SERVERS], total: 0, ping_index: 0, last_send: now, request: ListRequest::Idle }
    }
}

#[derive(Component)]
struct ServerRows;

#[derive(Component, Clone, Copy)]
struct ServerRow(usize);

#[derive(Component, Clone, Copy)]
struct ServerCell(usize, usize);

#[derive(Component)]
struct BrowserStatus;

pub(super) struct ServerBrowserPlugin;

impl Plugin for ServerBrowserPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::ServerList), (reset_browser, server_list))
            .add_systems(Update, (browse, show_servers, show_status).chain().run_if(in_state(Screen::ServerList)))
            .add_observer(join_server);
    }
}

fn reset_browser(mut commands: Commands, time: Res<Time<Real>>) {
    commands.insert_resource(ServerBrowser::new(time.elapsed().as_millis() as u32));
}

fn server_list(mut commands: Commands) {
    let p = page(&mut commands, Screen::ServerList);
    text(&mut commands, p, "Join Game", 32.0, Color::WHITE);

    let panel = commands.spawn((Node { width: percent(75), flex_direction: FlexDirection::Column, row_gap: px(4), margin: UiRect::top(px(16)), ..default() }, ChildOf(p))).id();
    let header = commands.spawn((Node { padding: UiRect { left: px(6), right: px(6.0 + ui::BAR), ..default() }, ..row_node() }, ChildOf(panel))).id();

    for (k, title) in std::iter::once("Name").chain(COLUMNS.iter().map(|c| c.0)).enumerate() {
        let cell = commands.spawn((cell_node(k), ChildOf(header))).id();

        commands.spawn((Text::new(title), TextFont::from_font_size(18.0), TextColor(HEADING), ChildOf(cell)));
    }

    let frame = commands.spawn((Node { height: px(LIST.y), ..default() }, ChildOf(panel))).id();
    let rows = commands
        .spawn((
            Node { flex_grow: 1.0, height: percent(100), flex_direction: FlexDirection::Column, row_gap: px(2), overflow: Overflow::scroll_y(), ..default() },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.3)),
            ScrollArea,
            ServerRows,
            ChildOf(frame),
        ))
        .id();
    ui::scrollbar(&mut commands, frame, rows);

    commands.spawn((Text::default(), TextFont::from_font_size(14.0), TextColor(HEADING), BrowserStatus, ChildOf(panel)));
    back_button(&mut commands, p);
}

fn row_node() -> Node {
    Node { width: percent(100), height: px(ROW_HEIGHT), flex_shrink: 0.0, padding: UiRect::horizontal(px(6)), align_items: AlignItems::Center, ..default() }
}

fn cell_node(column: usize) -> Node {
    match column {
        0 => Node { flex_grow: 1.0, overflow: Overflow::clip(), ..default() },
        k => Node { width: px(COLUMNS[k - 1].1), flex_shrink: 0.0, justify_content: JustifyContent::FlexEnd, ..default() },
    }
}

fn browse(time: Res<Time<Real>>, master: Option<Res<MasterAuth>>, mut socket: ResMut<ClientSocket>, mut list: ResMut<ServerBrowser>) {
    let now = time.elapsed().as_millis() as u32;
    let signed_in = master.as_ref().is_some_and(|m| m.state == AuthState::Done);

    if list.request == ListRequest::Idle && signed_in {
        list.request = ListRequest::Asking;
    }

    if now.wrapping_sub(list.last_send) > SEND_EVERY {
        list.last_send = now;

        if list.total > 0 {
            if list.total > list.ping_index {
                let ping = InfoRequest { version: 38 as u8, timestamp: now };

                socket.send(&frame_packet(ping), list.addresses[list.ping_index]);
            }

            list.ping_index += 1;

            if list.ping_index >= list.total + 10 {
                list.ping_index = 0;
            }
        }

        if list.request == ListRequest::Asking
            && let Some(m) = &master
        {
            socket.send(&frame_packet(ServerListRequest), m.server);
        }
    }

    if !signed_in {
        return;
    }

    while let Some(packet) = socket.recv() {
        let mut r = Reader::new(&packet.data[4..]);

        match r.u8() {
            Ok(1) => {
                let SocketAddr::V4(from) = packet.from else { continue };
                let Ok(info) = ServerInfo::read(&mut r) else { continue };

                let k = match list.entries.iter().position(|e| e.addr == from) {
                    Some(k) => k,
                    None => {
                        let mut e = ServerEntry::new(from);
                        e.name = info.server_name.clone();
                        e.id = info.id as u32;

                        if info.build > 0x1a {
                            e.advertised = SocketAddrV4::new(Ipv4Addr::from(u32::from_le_bytes(info.address)), info.port);
                        }

                        e.password = info.password_protected;
                        e.sub_version = info.sub_version as i32;

                        list.entries.push(e);
                        list.entries.len() - 1
                    }
                };

                let e = &mut list.entries[k];
                e.version = info.build as i32;
                e.ping = (time.elapsed().as_millis() as u32).wrapping_sub(info.timestamp);
                e.game_type = info.gamemode as i32;
                e.players = info.current_players as i32;
                e.max_players = info.max_players as i32;
            }
            Ok(0x4a) => {
                list.request = ListRequest::Received;
                let Ok(page) = ServerListPacket::read(&mut r) else { continue };
                list.total = (page.total as usize).min(MAX_SERVERS);
                for (j, addr) in (page.offset as usize..).zip(page.servers) {
                    if j < MAX_SERVERS {
                        list.addresses[j] = addr;
                    }
                }
            }
            _ => {}
        }
    }
}

fn show_servers(
    mut commands: Commands,
    list: Res<ServerBrowser>,
    containers: Query<Entity, With<ServerRows>>,
    rows: Query<&ServerRow>,
    mut cells: Query<(&ServerCell, &mut Text, &mut TextColor)>,
) {
    if !list.is_changed() {
        return;
    }

    let Some(container) = containers.iter().next() else { return };
    let shown = rows.iter().count();

    for (k, entry) in list.entries.iter().enumerate().skip(shown) {
        let row = commands.spawn((row_node(), BackgroundColor(Color::NONE), Hovered::default(), ServerRow(k), ChildOf(container))).id();

        if entry.joinable() {
            commands.entity(row).insert((Button, TabIndex(0)));
        } else {
            commands.entity(row).insert(InteractionDisabled);
        }

        for column in 0..6 {
            let cell = commands.spawn((cell_node(column), Pickable::IGNORE, ChildOf(row))).id();
            commands.spawn((Text::default(), TextFont::from_font_size(ROW_TEXT), TextColor(Color::WHITE), ServerCell(k, column), Pickable::IGNORE, ChildOf(cell)));
        }
    }

    for (cell, mut text, mut colour) in &mut cells {
        let Some(entry) = list.entries.get(cell.0) else { continue };
        let want = &entry.cells()[cell.1];

        if text.0 != *want {
            text.0.clone_from(want);
        }

        let shade = if entry.joinable() { Color::WHITE } else { OTHER_VERSION };

        if colour.0 != shade {
            colour.0 = shade;
        }
    }
}

fn show_status(list: Res<ServerBrowser>, master: Option<Res<MasterAuth>>, mut status: Query<&mut Text, With<BrowserStatus>>) {
    let signed_in = master.as_ref().is_some_and(|m| m.state == AuthState::Done);

    let want = if !signed_in {
        "Signing in to the masterserver...".to_string()
    } else if list.request != ListRequest::Received {
        "Asking for the server list...".to_string()
    } else {
        format!("{} servers listed, {} answered, {} not joinable", list.total, list.entries.len(), list.entries.iter().filter(|c| !c.joinable()).count())
    };

    for mut text in &mut status {
        if text.0 != want {
            text.0.clone_from(&want);
        }
    }
}

fn join_server(activate: On<Activate>, rows: Query<&ServerRow>, list: Option<Res<ServerBrowser>>, mut next: ResMut<NextState<Screen>>, mut commands: Commands) {
    let (Ok(&ServerRow(k)), Some(list)) = (rows.get(activate.entity), list) else { return };

    let Some(e) = list.entries.get(k) else { return };

    info!("joining {} at {}", e.name, e.addr);

    commands.insert_resource(JoinTarget { name: e.name.clone(), addr: e.addr, advertised: e.advertised, id: e.id, password: e.password });
    commands.insert_resource(Join::start(e.password));
    next.set(Screen::Connecting);
}
