use std::f32::consts::{PI, TAU};

use bevy::prelude::*;
use ferrosa_protocol::{
    GameMode, Team,
    clientbound::game::{GameState, ItemKind, MenuType, OwnHumanData},
};

use super::{
    GAME_FPS, GAME_SCREEN,
    controls::{InputState, flag},
    escape_menu::{BACKDROP_LAYER, GameMenu, OVERLAY_LAYER},
    items::ItemTypes,
    netgame::{Human, NetGame, Player},
};
use crate::{
    fonts::GameFonts,
    menu::{MENU_BACKGROUND, Screen},
};

const DRIVING: u32 = 2;
const PAIN_PER_SHADE: f32 = 64.0;
const DARKEST: f32 = 0.75;
const DAMAGE_LAYER: i32 = 1;
const FLYING: u32 = 3;
const GEARS: [&str; 3] = ["Run", "Walk", "Aim"];
const PHONE_LINES: [&str; 3] = ["1111 - Goldmen", "2222 - Monsota", "3333 - OXS"];
const WORLD_PHONE_LINES: [&str; 3] = ["4444 - Nexaco", "5555 - Pentacom", "6666 - Prodocon"];
const READY: i32 = 0;
pub const DIALLING: i32 = 1;
const RINGING: i32 = 2;
const IN_USE: i32 = 3;
const BUSY: i32 = 4;
const GEAR_LABELS: [(&str, f32, f32); 6] = [("1", 818.0, 420.0), ("2", 818.0, 476.0), ("3", 850.0, 420.0), ("4", 850.0, 476.0), ("A", 882.0, 420.0), ("R", 882.0, 476.0)];
const AIM_PIVOT: Vec2 = Vec2::new(608.0, 352.0);
const AIM_LIMITS: [Vec2; 2] = [Vec2::new(578.435_85, 327.508_27), Vec2::new(578.435_85, 376.491_73)];
const AIM_NEEDLE: f32 = 64.0;
const CORPORATION_MENUS: std::ops::RangeInclusive<i32> = 0xe..=0x1a;
const TIMER_AT: Vec2 = Vec2::new(1020.0, 4.0);

pub mod align {
    pub const CENTRE_X: u32 = 1;
    pub const RIGHT: u32 = 2;
    pub const CENTRE_Y: u32 = 4;
    pub const BOTTOM: u32 = 8;
    pub const TOP_LEFT: u32 = 0;
    pub const CENTRE: u32 = CENTRE_X | CENTRE_Y;
    pub const BOTTOM_CENTRE: u32 = CENTRE_X | BOTTOM;
}

#[derive(Component)]
pub struct HudScreen;

#[derive(Clone, Copy)]
pub enum Stick {
    Centre,
    Top,
    Bottom,
    BottomLeft,
    TopLeft,
    TopRight,
}

#[derive(Component)]
struct DamageFade;

#[derive(Component)]
struct ProgressBar;

#[derive(Component)]
struct ProgressFill;

#[derive(Component)]
struct StaminaBar;

#[derive(Component)]
struct StaminaFill;

#[derive(Component)]
struct StaminaMax;

#[derive(Component)]
struct GearText;

#[derive(Component)]
struct TimerScreen;

#[derive(Component)]
struct Backdrop;

#[derive(Component)]
struct RoundTimer;

#[derive(Component)]
struct MenuLine;

#[derive(Component)]
struct SharesLine;

#[derive(Component)]
struct BudgetLine;

#[derive(Component)]
struct PhoneHelp;

#[derive(Component)]
struct Dot;

#[derive(Component)]
struct GunTick(usize);

#[derive(Component)]
struct AimLine(usize);

#[derive(Component)]
struct CarGauges;

#[derive(Component)]
struct Needle(usize);

#[derive(Component)]
struct GearKnob;

#[derive(Component)]
struct Collective;

#[derive(Component)]
struct CollectiveLever;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::InGame), spawn_hud).add_systems(
            Update,
            (show_hud, (show_status, show_menu_line, show_phone_help, show_crosshair, show_aim, show_gauges, fade_for_damage, show_timer, show_backdrop).run_if(resource_exists::<NetGame>)).chain().run_if(in_state(Screen::InGame)),
        );
    }
}

pub fn own(net: &NetGame) -> Option<(&Player, &Human, &OwnHumanData)> {
    let p = net.me()?;
    let h = usize::try_from(p.human).ok().and_then(|h| net.humans.get(h))?;

    Some((p, h, net.own_human.as_ref()?))
}

pub fn in_hand(net: &NetGame, h: &Human, hand: usize) -> Option<(usize, ItemKind)> {
    let item = usize::try_from(*h.inventory[hand].first()?).ok()?;

    Some((item, net.items.get(item)?.kind?))
}

pub fn is_phone(kind: ItemKind) -> bool {
    matches!(kind, ItemKind::Phone | ItemKind::PhonePay)
}

pub fn colour([r, g, b, a]: [f32; 4]) -> Color {
    Color::srgba(r, g, b, a)
}

pub fn rect(flags: u32, at: Vec2, size: Vec2) -> Node {
    let x = at.x - if flags & align::CENTRE_X != 0 { size.x * 0.5 } else { 0.0 } - if flags & align::RIGHT != 0 { size.x } else { 0.0 };
    let y = at.y - if flags & align::CENTRE_Y != 0 { size.y * 0.5 } else { 0.0 } - if flags & align::BOTTOM != 0 { size.y } else { 0.0 };

    Node { position_type: PositionType::Absolute, left: px(x), top: px(y), width: px(size.x), height: px(size.y), ..default() }
}

pub fn anchored(flags: u32, at: Vec2) -> (Node, UiTransform) {
    let x = if flags & align::CENTRE_X != 0 { -50.0 } else if flags & align::RIGHT != 0 { -100.0 } else { 0.0 };
    let y = if flags & align::CENTRE_Y != 0 { -50.0 } else if flags & align::BOTTOM != 0 { -100.0 } else { 0.0 };
    let node = Node { position_type: PositionType::Absolute, left: px(at.x), top: px(at.y), ..default() };

    (node, UiTransform::from_translation(Val2::new(percent(x), percent(y))))
}

pub fn text(s: impl Into<String>, font: &Handle<Font>, size: f32, colour: Color) -> impl Bundle {
    (Text::new(s), TextFont::from_font_size(size).with_font(font.clone()), TextColor(colour), Pickable::IGNORE)
}

pub fn line(a: Vec2, b: Vec2, width: f32, round: bool) -> (Node, UiTransform) {
    let d = b - a;
    let length = d.length() + if round { width } else { 0.0 };
    let mid = (a + b) * 0.5;
    let radius = if round { px(width * 0.5) } else { px(0) };

    let node = Node {
        position_type: PositionType::Absolute,
        left: px(mid.x - length * 0.5),
        top: px(mid.y - width * 0.5),
        width: px(length),
        height: px(width),
        border_radius: BorderRadius::all(radius),
        ..default()
    };

    (node, UiTransform::from_rotation(Rot2::radians(d.y.atan2(d.x))))
}

pub fn spawn_screen(commands: &mut Commands, marker: impl Bundle, stick: Stick) -> Entity {
    let (across, down) = match stick {
        Stick::Centre => (JustifyContent::Center, AlignItems::Center),
        Stick::Bottom => (JustifyContent::Center, AlignItems::FlexEnd),
        Stick::BottomLeft => (JustifyContent::FlexStart, AlignItems::FlexEnd),
        Stick::TopLeft => (JustifyContent::FlexStart, AlignItems::FlexStart),
        Stick::Top => (JustifyContent::Center, AlignItems::FlexStart),
        Stick::TopRight => (JustifyContent::FlexEnd, AlignItems::FlexStart),
    };
    let full = Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), justify_content: across, align_items: down, ..default() };
    let window = commands.spawn((full, Pickable::IGNORE, DespawnOnExit(Screen::InGame))).id();
    let screen = Node { width: px(GAME_SCREEN.x), height: px(GAME_SCREEN.y), ..default() };

    commands.spawn((screen, marker, Pickable::IGNORE, Visibility::Hidden, ChildOf(window))).id()
}

fn spawn_hud(mut commands: Commands, fonts: Res<GameFonts>) {
    let centre = spawn_screen(&mut commands, HudScreen, Stick::Centre);
    let bottom = spawn_screen(&mut commands, HudScreen, Stick::Bottom);
    let bottom_left = spawn_screen(&mut commands, HudScreen, Stick::BottomLeft);
    let top_left = spawn_screen(&mut commands, HudScreen, Stick::TopLeft);
    let white = Color::WHITE;

    commands.spawn((rect(align::TOP_LEFT, Vec2::new(384.0, 288.0), Vec2::new(256.0, 32.0)), ProgressBar, BackgroundColor(colour([1.0, 1.0, 1.0, 0.125])), ChildOf(centre)));
    commands.spawn((rect(align::TOP_LEFT, Vec2::new(384.0, 288.0), Vec2::new(0.0, 32.0)), ProgressFill, BackgroundColor(colour([1.0, 1.0, 1.0, 0.875])), ChildOf(centre)));

    commands.spawn((rect(align::BOTTOM, Vec2::new(24.0, 540.0), Vec2::new(24.0, 127.0)), StaminaBar, BackgroundColor(colour([1.0, 1.0, 1.0, 0.125])), ChildOf(bottom_left)));
    commands.spawn((rect(align::BOTTOM, Vec2::new(24.0, 540.0), Vec2::new(24.0, 0.0)), StaminaFill, BackgroundColor(colour([1.0, 1.0, 1.0, 0.5])), ChildOf(bottom_left)));
    commands.spawn((rect(align::CENTRE_Y, Vec2::new(24.0, 540.0), Vec2::new(24.0, 8.0)), StaminaMax, BackgroundColor(colour([1.0, 1.0, 1.0, 0.875])), ChildOf(bottom_left)));

    let (node, shift) = anchored(align::BOTTOM, Vec2::new(64.0, 572.0));
    commands.spawn((node, shift, GearText, text("", &fonts.rockwell, 24.0, white), ChildOf(bottom_left)));

    let (node, shift) = anchored(align::TOP_LEFT, Vec2::new(4.0, 4.0));
    commands.spawn((node, shift, MenuLine, text("", &fonts.rockwell, 24.0, white), ChildOf(top_left)));
    let (node, shift) = anchored(align::TOP_LEFT, Vec2::new(192.0, 4.0));
    commands.spawn((node, shift, SharesLine, text("", &fonts.rockwell, 24.0, white), ChildOf(top_left)));
    let (node, shift) = anchored(align::TOP_LEFT, Vec2::new(512.0, 4.0));
    commands.spawn((node, shift, BudgetLine, text("", &fonts.rockwell, 24.0, white), ChildOf(top_left)));

    let (node, shift) = anchored(align::CENTRE_X, Vec2::new(512.0, 384.0));
    commands.spawn((Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..node }, shift, PhoneHelp, ChildOf(centre)));

    commands.spawn((rect(align::CENTRE, Vec2::new(512.0, 288.0), Vec2::splat(4.0)), Dot, BackgroundColor(colour([1.0, 1.0, 1.0, 0.5])), ChildOf(centre)));

    for side in 0..4 {
        commands.spawn((Node::default(), GunTick(side), BackgroundColor(colour([1.0, 1.0, 1.0, 0.5])), ChildOf(centre)));
    }

    for (k, shade) in [[1.0, 1.0, 1.0, 0.5], [1.0, 1.0, 1.0, 0.5], [1.0, 1.0, 0.0, 0.75], [1.0, 1.0, 0.0, 0.5], [1.0, 1.0, 0.0, 0.5]].into_iter().enumerate() {
        commands.spawn((Node::default(), AimLine(k), BackgroundColor(colour(shade)), Visibility::Hidden, ChildOf(centre)));
    }

    spawn_gauges(&mut commands, bottom, &fonts.rockwell);

    let top_right = spawn_screen(&mut commands, TimerScreen, Stick::TopRight);
    commands.entity(top_right).insert(GlobalZIndex(OVERLAY_LAYER));

    let backdrop = Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() };
    commands.spawn((backdrop, Backdrop, BackgroundColor(MENU_BACKGROUND), Visibility::Hidden, GlobalZIndex(BACKDROP_LAYER), Pickable::IGNORE, DespawnOnExit(Screen::InGame)));
    let (node, shift) = anchored(align::RIGHT, TIMER_AT);
    commands.spawn((node, shift, RoundTimer, text("", &fonts.osd_white, 24.0, white), ChildOf(top_right)));

    let full = Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() };
    commands.spawn((full, DamageFade, BackgroundColor(Color::NONE), GlobalZIndex(DAMAGE_LAYER), Pickable::IGNORE, DespawnOnExit(Screen::InGame)));
}

fn show_backdrop(net: Res<NetGame>, mut backdrop: Single<&mut Visibility, With<Backdrop>>) {
    let lobby = net.me().is_some_and(|p| p.menu == MenuType::Lobby);

    shown(&mut backdrop, lobby || net.game_state == GameState::Restarting);
}

fn fade_for_damage(net: Res<NetGame>, mut fade: Single<&mut BackgroundColor, With<DamageFade>>) {
    let dark = own(&net).map_or(0.0, |(_, _, own)| (own.pain.max(0) as f32 / PAIN_PER_SHADE).min(DARKEST));
    let colour = Color::BLACK.with_alpha(dark);

    if fade.0 != colour {
        fade.0 = colour;
    }
}

fn clock(ticks: i32) -> String {
    let seconds = ticks.max(0) / GAME_FPS as i32;

    format!("{}:{:02}", seconds / 60, seconds % 60)
}

fn show_timer(net: Res<NetGame>, mut screen: Single<&mut Visibility, With<TimerScreen>>, mut timer: Single<&mut Text, With<RoundTimer>>) {
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

fn spawn_gauges(commands: &mut Commands, screen: Entity, font: &Handle<Font>) {
    let shade = BackgroundColor(colour([0.0, 0.0, 0.0, 0.75]));
    let car = commands.spawn((Node { position_type: PositionType::Absolute, ..default() }, CarGauges, Pickable::IGNORE, Visibility::Hidden, ChildOf(screen))).id();

    for (k, (centre, radius)) in [(Vec2::new(548.0, 480.0), 36.0), (Vec2::new(480.0, 480.0), 32.0)].into_iter().enumerate() {
        let dial = Node { border: UiRect::all(px(2)), border_radius: BorderRadius::all(percent(50)), ..rect(align::CENTRE, centre, Vec2::splat(radius * 2.0)) };
        commands.spawn((dial, BackgroundColor(colour([0.0, 0.0, 0.0, 0.25])), BorderColor::all(Color::BLACK), ChildOf(car)));

        let (node, turn) = line(centre, centre + Vec2::new(0.0, -radius), 1.5, false);
        commands.spawn((node, turn, Needle(k), BackgroundColor(colour([0.75, 0.75, 0.75, 1.0])), ChildOf(car)));
    }

    for x in [818.0, 850.0, 882.0] {
        commands.spawn((rect(align::CENTRE, Vec2::new(x, 448.0), Vec2::new(8.0, 68.0)), shade, ChildOf(car)));
    }

    commands.spawn((rect(align::CENTRE, Vec2::new(850.0, 448.0), Vec2::new(72.0, 8.0)), shade, ChildOf(car)));

    for (label, x, y) in GEAR_LABELS {
        let (node, shift) = anchored(align::CENTRE, Vec2::new(x, y));
        commands.spawn((node, shift, text(label, font, 16.0, colour([1.0, 1.0, 1.0, 0.75])), ChildOf(car)));
    }

    let knob = Node { border_radius: BorderRadius::all(percent(50)), ..rect(align::CENTRE, Vec2::new(818.0, 448.0), Vec2::splat(24.0)) };
    commands.spawn((knob, GearKnob, BackgroundColor(colour([1.0, 1.0, 1.0, 0.75])), ChildOf(car)));

    commands.spawn((rect(align::CENTRE, Vec2::new(818.0, 448.0), Vec2::new(16.0, 80.0)), Collective, BackgroundColor(colour([0.0, 0.0, 0.0, 0.5])), Visibility::Hidden, ChildOf(screen)));
    commands.spawn((rect(align::CENTRE, Vec2::new(818.0, 448.0), Vec2::splat(16.0)), CollectiveLever, BackgroundColor(colour([1.0, 1.0, 1.0, 0.5])), Visibility::Hidden, ChildOf(screen)));
}

fn shown(visibility: &mut Mut<Visibility>, show: bool) {
    visibility.set_if_neq(if show { Visibility::Inherited } else { Visibility::Hidden });
}

fn show_hud(net: Option<Res<NetGame>>, mut screens: Query<&mut Visibility, With<HudScreen>>) {
    let ours = net.is_some_and(|n| own(&n).is_some());

    screens.iter_mut().for_each(|mut v| shown(&mut v, ours));
}

#[allow(clippy::type_complexity)]
fn show_status(
    net: Res<NetGame>,
    mut bars: ParamSet<(
        Query<(Has<ProgressBar>, &mut Visibility), Or<(With<ProgressBar>, With<StaminaBar>)>>,
        Query<(&mut Node, &mut Visibility), With<ProgressFill>>,
        Query<(&mut Node, &mut Visibility), With<StaminaFill>>,
        Query<(&mut Node, &mut Visibility), With<StaminaMax>>,
        Query<(&mut Text, &mut Visibility), With<GearText>>,
    )>,
) {
    let Some((p, h, own)) = own(&net) else { return };
    let playing = net.game_state != GameState::Restarting;
    let progress = playing && own.progress_bar > 0;

    for (progress_bar, mut v) in &mut bars.p0() {
        shown(&mut v, if progress_bar { progress } else { playing });
    }

    for (mut node, mut v) in &mut bars.p1() {
        shown(&mut v, progress);
        node.width = px(own.progress_bar as f32);
    }

    for (mut node, mut v) in &mut bars.p2() {
        shown(&mut v, playing);
        node.height = px(own.stamina as f32);
        node.top = px(540.0 - own.stamina as f32);
    }

    for (mut node, mut v) in &mut bars.p3() {
        shown(&mut v, playing);
        node.top = px(540.0 - own.max_stamina.min(0x7f) as f32 - 4.0);
    }

    for (mut t, mut v) in &mut bars.p4() {
        let gear = GEARS.get(p.zoom_level as usize).filter(|_| playing && h.vehicle < 0);

        shown(&mut v, gear.is_some());

        if let Some(gear) = gear
            && t.0 != *gear
        {
            t.0 = gear.to_string();
        }
    }
}

fn set_text(t: &mut Text, v: &mut Mut<Visibility>, s: Option<String>) {
    shown(v, s.is_some());

    if let Some(s) = s
        && t.0 != s
    {
        t.0 = s;
    }
}

#[allow(clippy::type_complexity)]
fn show_menu_line(
    net: Res<NetGame>,
    menu: Res<State<GameMenu>>,
    mut lines: ParamSet<(
        Query<(&mut Text, &mut Node, &mut Visibility), With<MenuLine>>,
        Query<(&mut Text, &mut Node, &mut Visibility), With<SharesLine>>,
        Query<(&mut Text, &mut Visibility), With<BudgetLine>>,
    )>,
) {
    let Some((p, _, _)) = own(&net) else { return };
    let open = *menu.get() == GameMenu::PlayerMenu;
    let world = net.sync.game_type == GameMode::World;
    let corporation = world && CORPORATION_MENUS.contains(&(p.menu as i32)) && p.team != Team::Spectator;
    let has_menu = p.menu != MenuType::Empty;

    let price = Team::CORPORATIONS.iter().position(|&t| t == p.team).map(|t| net.share_prices[t]);
    let hint = has_menu && !open;
    let wealth = has_menu && open && !world;

    let first = if hint { Some("Press Tab for Menu".to_string()) } else { wealth.then(|| format!("${}", p.money)) };
    let second = if corporation { Some(format!("${}", p.team_money)) } else { price.filter(|_| wealth).map(|price| format!("${}", (price / 10).wrapping_mul(p.stocks))) };
    let third = corporation.then(|| format!("Budget: ${}", p.budget));

    for (mut t, mut node, mut v) in &mut lines.p0() {
        node.top = px(4.0);
        set_text(&mut t, &mut v, first.clone());
    }

    for (mut t, mut node, mut v) in &mut lines.p1() {
        node.left = px(if corporation { 196.0 } else { 192.0 });
        set_text(&mut t, &mut v, second.clone());
    }

    for (mut t, mut v) in &mut lines.p2() {
        set_text(&mut t, &mut v, third.clone());
    }
}

fn phone_help(net: &NetGame, item: usize) -> Vec<(String, f32, Color)> {
    let (status, number, _) = net.items[item].phone;
    let white = Color::WHITE;
    let line = |s: &str| (s.to_string(), 16.0, white);

    let mut lines = match status {
        READY => vec![line("Left-click to dial")],
        DIALLING => {
            let world = if net.sync.game_type == GameMode::World { &WORLD_PHONE_LINES[..] } else { &[] };
            [&["Use Numpad or number keys"][..], &PHONE_LINES, world].concat().into_iter().map(line).collect()
        }
        BUSY => vec![line("Busy")],
        RINGING => vec![line(if number > 0 { "Left-click to answer" } else { "Left-click to hang up" })],
        IN_USE => vec![("In Use".to_string(), 24.0, colour([0.0, 1.0, 0.0, 1.0]))],
        _ => vec![],
    };

    if matches!(status, DIALLING | RINGING) && number > 0 {
        lines.push((format!("Number: {number}"), 24.0, white));
    }

    lines
}

fn show_phone_help(
    mut commands: Commands,
    net: Res<NetGame>,
    fonts: Res<GameFonts>,
    help: Single<(Entity, Option<&Children>), With<PhoneHelp>>,
    mut last: Local<Vec<(String, f32, Color)>>,
) {
    let lines = own(&net)
        .filter(|_| net.game_state != GameState::Restarting)
        .and_then(|(_, h, _)| in_hand(&net, h, 0))
        .filter(|&(_, kind)| is_phone(kind))
        .map_or_else(Vec::new, |(item, _)| phone_help(&net, item));

    if *last == lines {
        return;
    }

    let (help, children) = *help;
    children.into_iter().flatten().for_each(|&c| commands.entity(c).despawn());

    for (s, size, colour) in &lines {
        commands.spawn((text(s.clone(), &fonts.rockwell, *size, *colour), ChildOf(help)));
    }

    *last = lines;
}

fn show_crosshair(net: Res<NetGame>, types: Res<ItemTypes>, mut dot: Query<&mut Visibility, (With<Dot>, Without<GunTick>)>, mut ticks: Query<(&GunTick, &mut Node, &mut UiTransform, &mut Visibility)>) {
    let ours = own(&net).filter(|_| net.game_state != GameState::Restarting);
    let on_foot = ours.is_some_and(|(_, h, _)| h.vehicle < 0);

    dot.iter_mut().for_each(|mut v| shown(&mut v, on_foot));

    let gun = ours.filter(|(p, _, _)| p.input_flags & flag::SHIFT == 0).and_then(|(p, _, _)| {
        let held = p.display_inventory[0].first().map(|&c| (c & 0xff) as usize)?;
        types.0.get(held).filter(|t| t.is_gun).map(|_| 8.0 / (1.0 - p.zoom_eased))
    });

    for (&GunTick(side), mut node, mut turn, mut v) in &mut ticks {
        shown(&mut v, gun.is_some());

        let Some(gap) = gun else { continue };
        let centre = Vec2::new(512.0, 288.0);
        let toward = [Vec2::NEG_Y, Vec2::Y, Vec2::NEG_X, Vec2::X][side];
        let (n, t) = line(centre + toward * gap, centre + toward * gap * 2.0, 1.0, false);

        *node = n;
        *turn = t;
    }
}

fn show_aim(net: Res<NetGame>, input: Res<InputState>, mut lines: Query<(&AimLine, &mut Node, &mut UiTransform, &mut Visibility)>) {
    let pitch = own(&net).filter(|_| net.game_state != GameState::Restarting && input.aiming()).map(|(p, _, _)| p.free_look_pitch);
    let tip = |pitch: f32, scale: f32| {
        let a = 15.0 * pitch * scale;
        AIM_PIVOT + Vec2::new(-a.cos() * AIM_NEEDLE * 0.5, a.sin() * AIM_NEEDLE)
    };

    for (&AimLine(k), mut node, mut turn, mut v) in &mut lines {
        shown(&mut v, pitch.is_some());

        let Some(pitch) = pitch else { continue };
        let (a, b) = match k {
            0 | 1 => (AIM_PIVOT, AIM_LIMITS[k]),
            2 => (AIM_PIVOT, tip(pitch, 1.0)),
            3 => (tip(pitch, 1.0), tip(pitch, 0.9375)),
            _ => (tip(pitch, 0.9375), tip(pitch, 0.875)),
        };

        (*node, *turn) = line(a, b, 1.0, false);
    }
}

#[allow(clippy::type_complexity)]
fn show_gauges(
    net: Res<NetGame>,
    mut car: Query<&mut Visibility, (With<CarGauges>, Without<Collective>, Without<CollectiveLever>)>,
    mut needles: Query<(&Needle, &mut Node, &mut UiTransform), Without<GearKnob>>,
    mut knob: Query<&mut Node, (With<GearKnob>, Without<Needle>, Without<CollectiveLever>)>,
    mut collective: Query<&mut Visibility, (With<Collective>, Without<CarGauges>, Without<CollectiveLever>)>,
    mut lever: Query<(&mut Node, &mut Visibility), (With<CollectiveLever>, Without<Needle>, Without<GearKnob>)>,
) {
    let driving = own(&net).filter(|(_, h, _)| net.game_state != GameState::Restarting && h.vehicle >= 0 && h.vehicle_seat == 0);
    let input = driving.map_or(0, |(p, _, _)| p.last_input_type);

    car.iter_mut().for_each(|mut v| shown(&mut v, input == DRIVING));
    collective.iter_mut().for_each(|mut v| shown(&mut v, input == FLYING));

    for (mut node, mut v) in &mut lever {
        shown(&mut v, input == FLYING);

        if let Some((p, _, _)) = driving {
            node.top = px(448.0 - 32.0 * p.forward_back - 8.0);
        }
    }

    let Some((p, h, _)) = driving.filter(|_| input == DRIVING) else { return };
    let Some(v) = net.vehicles.get(h.vehicle as usize) else { return };

    let ticks = (v.track.tick - v.track.prev_tick).max(1) as f32;
    let speed = (v.track.pos - v.track.prev_pos).length() / ticks;
    let mph = speed * 60.0 * 60.0 * 60.0 * 0.621_371_2 / 1000.0 / 180.0;
    let turns = [mph * TAU, v.engine_rpm as f32 / 5000.0 * PI];

    for (&Needle(k), mut node, mut turn) in &mut needles {
        let (centre, radius) = [(Vec2::new(548.0, 480.0), 36.0), (Vec2::new(480.0, 480.0), 32.0)][k];
        let angle = turns[k] - PI * 0.75;
        let (n, t) = line(centre, centre + Vec2::new(angle.sin(), -angle.cos()) * radius, 1.5, false);

        *node = n;
        *turn = t;
    }

    for mut node in &mut knob {
        node.left = px(818.0 + 32.0 * p.gear_x - 12.0);
        node.top = px(448.0 + 32.0 * p.gear_y - 12.0);
    }
}
