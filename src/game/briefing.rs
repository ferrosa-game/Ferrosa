use bevy::prelude::*;
use ferrosa_protocol::{
    GameMode, Team,
    clientbound::game::{GameState, MenuType},
};

use super::{
    Level,
    escape_menu::OVERLAY_LAYER,
    hud::{Stick, align, anchored, colour, rect, spawn_screen, text},
    items::ItemTypes,
    netgame::{Mission, NetGame, PROTECTOR, entities::Player},
};
use crate::{
    fonts::GameFonts,
    menu::Screen,
    names::{ROUND_CORPORATIONS, corporation_name, phone_number},
};

const WHITE: [f32; 4] = [1.0; 4];
const WANTED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
const TICKS_PER_HOUR: u32 = 216000;
const TICKS_PER_MINUTE: u32 = 3600;
const LUNCH: std::ops::RangeInclusive<u32> = 0x278d00..=0x278d00 + 0x34bbf;
const MISSION_SPOTS: [&str; 8] = ["Hondo Park", "Rio Granary", "Isle of Burgers", "Park above the Kamel Building", "Bobson Dugnutt Museum", "The Mall", "Gas Station", "Red Cube Park"];
const ROLES: [(&str, [f32; 3]); 3] = [("You are a Civilian", [0.75, 0.75, 0.75]), ("You are the Eliminator", [0.75, 0.0, 0.0]), ("You are the Protector", [0.75, 0.75, 1.0])];

#[derive(Clone, PartialEq)]
pub struct Line {
    pub text: String,
    pub at: Vec2,
    pub flags: u32,
    pub size: f32,
    pub colour: [f32; 4],
}

pub fn line(text: impl Into<String>, at: Vec2, flags: u32, size: f32, colour: [f32; 4]) -> Line {
    Line { text: text.into(), at, flags, size, colour }
}

#[derive(Component, Default)]
pub struct TextLayer(Option<Vec<Line>>);

pub fn redraw(commands: &mut Commands, screen: Entity, layer: &mut TextLayer, font: &Handle<Font>, lines: Option<Vec<Line>>, backing: Option<(Vec2, Vec2, [f32; 4])>) {
    if layer.0 == lines {
        return;
    }

    commands.entity(screen).despawn_children();

    if let Some(lines) = &lines {
        if let Some((at, size, shade)) = backing {
            commands.spawn((rect(align::TOP_LEFT, at, size), BackgroundColor(colour(shade)), Pickable::IGNORE, ChildOf(screen)));
        }

        for l in lines {
            let (node, shift) = anchored(l.flags, l.at);
            commands.spawn((node, shift, text(l.text.clone(), font, l.size, colour(l.colour)), ChildOf(screen)));
        }
    }

    layer.0 = lines;
}

#[derive(Component)]
struct WorldCorner;

#[derive(Component)]
struct WorldClock;

#[derive(Component)]
struct CentreNotice;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum RowKind {
    Title,
    Heading,
    Body,
    Blank,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Row {
    pub kind: RowKind,
    pub text: String,
    pub colour: [f32; 4],
}

fn row(kind: RowKind, text: impl Into<String>) -> Row {
    Row { kind, text: text.into(), colour: WHITE }
}

pub struct BriefingPlugin;

impl Plugin for BriefingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::InGame), spawn_layers)
            .add_systems(Update, (show_world_hud, show_notice).run_if(in_state(Screen::InGame)).run_if(resource_exists::<NetGame>));
    }
}

fn spawn_layers(mut commands: Commands) {
    let layer = |commands: &mut Commands, marker: &dyn Fn(&mut EntityCommands), stick: Stick, z: Option<i32>| {
        let screen = spawn_screen(commands, TextLayer::default(), stick);
        let mut e = commands.entity(screen);
        e.insert(Visibility::Inherited);
        marker(&mut e);

        if let Some(z) = z {
            e.insert(GlobalZIndex(z));
        }
    };

    layer(&mut commands, &|e| { e.insert(WorldCorner); }, Stick::TopLeft, None);
    layer(&mut commands, &|e| { e.insert(WorldClock); }, Stick::TopRight, None);
    layer(&mut commands, &|e| { e.insert(CentreNotice); }, Stick::Centre, Some(OVERLAY_LAYER));
}

fn world(net: &NetGame) -> bool {
    net.sync.game_type == GameMode::World
}

fn clock(time: u32) -> String {
    let hour = match time / TICKS_PER_HOUR % 12 {
        0 => 12,
        h => h,
    };

    format!("{hour}:{:02}", time / TICKS_PER_MINUTE % 60)
}

fn show_world_hud(
    mut commands: Commands,
    net: Res<NetGame>,
    fonts: Res<GameFonts>,
    mut corner: Single<(Entity, &mut TextLayer), (With<WorldCorner>, Without<WorldClock>)>,
    mut top_right: Single<(Entity, &mut TextLayer), With<WorldClock>>,
) {
    let me = net.me().filter(|_| world(&net));
    let corner_lines = me.map(|p| {
        let mut lines = vec![line(format!("${}", p.money), Vec2::new(4.0, 4.0), align::TOP_LEFT, 24.0, WHITE)];

        if p.criminal_rating > 0 {
            lines.push(line(format!("Wanted Level:{}", p.criminal_rating), Vec2::new(768.0, 4.0), align::TOP_LEFT, 24.0, WANTED));
        }

        lines
    });
    let clock_lines = me.map(|_| vec![line(clock(net.sun_time), Vec2::new(1020.0, 4.0), align::RIGHT, 24.0, WHITE)]);

    let (e, layer) = &mut *corner;
    redraw(&mut commands, *e, layer, &fonts.rockwell, corner_lines, None);

    let (e, layer) = &mut *top_right;
    redraw(&mut commands, *e, layer, &fonts.rockwell, clock_lines, None);
}

fn savior(net: &NetGame) -> String {
    let k = usize::try_from(net.saviors.0).ok().filter(|&k| k < net.players.len()).unwrap_or(0);

    net.players[k].name.clone()
}

fn show_notice(mut commands: Commands, net: Res<NetGame>, fonts: Res<GameFonts>, mut notice: Single<(Entity, &mut TextLayer), With<CentreNotice>>) {
    let mut lines = Vec::new();

    if let Some(p) = net.me() {
        if world(&net) && p.menu == MenuType::EnterCity && p.spawn_timer > 0 {
            lines.push(line(format!("Respawn in {}", p.spawn_timer / 60), Vec2::new(512.0, 240.0), align::CENTRE_X, 24.0, WHITE));
        }

        let before = matches!(net.game_state, GameState::Idle | GameState::Intermission);

        if net.sync.game_type == GameMode::Eliminator && before
            && let Some(&(role, [r, g, b])) = usize::try_from(net.role).ok().and_then(|k| ROLES.get(k))
        {
            lines.push(line(role, Vec2::new(512.0, 272.0), align::CENTRE_X, 32.0, [r, g, b, 1.0]));

            if net.role == PROTECTOR {
                lines.push(line(format!("The Savior is {}", savior(&net)), Vec2::new(512.0, 304.0), align::CENTRE_X, 32.0, [r, g, b, 1.0]));
            }
        }
    }

    let (e, layer) = &mut *notice;
    redraw(&mut commands, *e, layer, &fonts.rockwell, (!lines.is_empty()).then_some(lines), None);
}

fn mission_team_name(team: i32) -> String {
    usize::try_from(team).ok().and_then(|t| Team::CORPORATION_NAMES.get(t)).map_or_else(|| format!("team {}", team + 1), |n| n.to_string())
}

fn info_card(p: &Player, rows: &mut Vec<Row>) {
    rows.push(row(RowKind::Title, p.name.clone()));
    rows.push(row(RowKind::Body, phone_number(p.phone_number as u32)));
    rows.push(row(RowKind::Body, format!("Corporate rating: {}", p.corporate_rating)));
    rows.push(row(RowKind::Blank, ""));

    match corporation_name(p.team) {
        Some(name) => {
            rows.push(row(RowKind::Heading, name));
            rows.push(row(RowKind::Body, format!("${}", p.team_money)));
            rows.push(row(RowKind::Body, format!("Budget: ${}", p.budget)));
        }
        None => rows.push(row(RowKind::Heading, "Freelancer")),
    }
}

fn mission_lines(m: &Mission, disk: &str, place: &dyn Fn(i32) -> Option<(String, String)>) -> Vec<String> {
    let acquisition = ["Acquisition:", "We have received reports that Megacorp has disks containing", "information that will be beneficial to one of our subsidiaries."];
    let mut out: Vec<String> = Vec::new();
    let mut add = |s: &str| out.push(s.to_string());

    match m.kind {
        0 => {
            acquisition.iter().for_each(|s| add(s));

            if m.location == -1 {
                add("We do not know where the disk is located.");
                add("");
            } else {
                add(&format!("The {disk} is in a building near the"));
                add(&place(m.location).map_or(String::new(), |(a, b)| format!("{a} and {b} intersection")));
            }

            add(&format!("Value:               ${} Bonus", m.value));
            add(&format!("Estimated stock increase:         {}%", ((m.value + 1) as f32 * (1.0 / 400.0)) as i32));
        }
        1 => {
            acquisition.iter().for_each(|s| add(s));
            add(&format!("The {disk} is in a town car driving around the city."));
            add(&format!("Value:               ${} Bonus", m.value));
        }
        2 => {
            add("Transaction:");

            if m.location != -1 {
                add("One of our agents has acquired a disk that has information");
                add("wanted by another corporation.  The disk is in a briefcase.");
                add(&format!("Dead drop location: {}", usize::try_from(m.location).ok().and_then(|k| MISSION_SPOTS.get(k)).copied().unwrap_or("")));
                add("");
            } else {
                add("We have acquired a disk that has information that can be");
                add("traded for money with another corporation.");
            }

            add("Negotiate the best terms and execute the deal.");
            add(&format!("{disk} value:               ${} Bonus", m.value));
        }
        3 | 4 => {
            add("Transaction:");
            add(&format!("We have received reports that {} has acquired a disk", mission_team_name(m.team1)));
            add("containing information that will be beneficial to one of our");

            if m.kind == 3 {
                add("subsidiaries.  Negotiate the best terms and execute the deal.");
                add(&format!("You have been provided with ${}.", m.cash));
            } else {
                add("subsidiaries.  Acquire the disk by any means neccessary.");
            }

            add(&format!("{disk} value:               ${} Bonus", m.value));
        }
        5 | 6 => {
            add("Three-way Deal:");
            add(&format!("We have received reports that {} has acquired a disk", mission_team_name(m.team1)));
            add("containing information that will be beneficial to one of our");

            if m.kind == 5 {
                add("subsidiaries.  We have a disk to trade with another");
                add("corporation.  Execute the deal.");
            } else {
                add("subsidiaries.  Acquire the disk by any means neccessary.");
            }

            add(&format!("{disk} value:               ${} Bonus", m.value));
        }
        7 => {
            add("Double Disk:");
            add("We have acquired a disk that contains some of the data");
            add(&format!("that {} is seeking.", mission_team_name(m.team1)));
            add("");
            add("Negotiate the best terms and execute the deal.");
            add(&format!("{disk}s value:               ${} Bonus", m.value));
        }
        8 | 9 => {
            add("Double Disk:");
            add(&format!("We have received reports that {} and", mission_team_name(m.team1)));
            add(&format!("{} have each acquired a disk containing", mission_team_name(m.team2)));
            add("part of the data for a new technology we need.");
            add("We require both disks, one alone is worthless.");
            add("");

            if m.kind == 8 {
                add("Negotiate the best terms and execute the deal.");
                add(&format!("You have been provided with ${}.", m.cash));
            } else {
                add("Acquire the disks by any means neccessary.");
            }

            add(&format!("{disk}s value:               ${} Bonus", m.value));
        }
        _ => {}
    }

    out
}

pub fn briefing_rows(net: &NetGame, level: Option<&Level>, types: Option<&ItemTypes>) -> Vec<Row> {
    let Some(p) = net.me() else { return Vec::new() };
    let mut rows = Vec::new();

    match net.sync.game_type {
        GameMode::Eliminator => match usize::try_from(net.role).ok().and_then(|k| ROLES.get(k)) {
            Some(&(role, [r, g, b])) => {
                rows.push(Row { kind: RowKind::Title, text: role.to_string(), colour: [r, g, b, 1.0] });

                if net.role == PROTECTOR {
                    rows.push(row(RowKind::Body, format!("The Savior is {}", savior(net))));
                }
            }
            None => rows.push(row(RowKind::Title, "Waiting for the round")),
        },
        GameMode::World => info_card(p, &mut rows),
        _ => match ROUND_CORPORATIONS.iter().position(|&t| t == p.team) {
            Some(team) => {
                rows.push(row(RowKind::Title, Team::CORPORATION_NAMES[team]));
                rows.push(row(RowKind::Blank, ""));

                let disk = |m: &Mission| usize::try_from(m.disk_type).ok().and_then(|k| types?.0.get(k)).map_or(String::new(), |t| t.name.clone());
                let place = |location: i32| {
                    let map = &level?.map;
                    let (a, b) = map.level.buildings.get(usize::try_from(location).ok()?)?.streets;
                    let street = |s: i32| map.streets.streets.get(usize::try_from(s).ok()?).map(|s| s.name.to_string());
                    Some((street(a)?, street(b)?))
                };
                let missions: Vec<&Mission> = net.missions.get(team).map(|m| m.iter().filter(|m| m.active).collect()).unwrap_or_default();

                for m in &missions {
                    let mut lines = mission_lines(m, &disk(m), &place).into_iter();

                    if let Some(heading) = lines.next() {
                        rows.push(row(RowKind::Heading, heading.trim_end_matches(':')));
                    }

                    rows.extend(lines.map(|l| if l.is_empty() { row(RowKind::Blank, "") } else { row(RowKind::Body, l) }));
                    rows.push(row(RowKind::Blank, ""));
                }

                if missions.is_empty() {
                    let lunch = LUNCH.contains(&net.sun_time);
                    rows.push(row(RowKind::Body, if lunch { "Lunch break" } else { "No current missions" }));
                }
            }
            None => rows.push(row(RowKind::Title, "Spectating")),
        },
    }

    rows
}
