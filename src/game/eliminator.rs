use bevy::prelude::*;
use ferrosa_protocol::{
    GameMode,
    clientbound::game::{GameState, MenuType},
};

use super::{
    camera::GameCamera,
    escape_menu::ROSTER_LAYER,
    hud::{Stick, align, anchored, colour, spawn_screen, text},
    mute::{Voice, VoiceIcons, voice_icon},
    netgame::{NetGame, PROTECTOR},
    smoothing::Smoothing,
};
use crate::{fonts::GameFonts, menu::Screen, names::phone_number};

const PALE: [f32; 4] = [0.75, 0.75, 1.0, 1.0];
const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
const WHITE: [f32; 4] = [1.0; 4];
const ICON: f32 = 24.0;
const ROW: f32 = 24.0;
const ELIMINATOR: i32 = 1;
const BOX_RED: [f32; 4] = [1.0, 0.0, 0.0, 0.75];
const BOX_GREEN: [f32; 4] = [0.0, 1.0, 0.0, 0.75];
const BOX_NEAR: f32 = 3072.0;
const BOX_MIN: f32 = 8.0;
const BOX_MAX: f32 = 192.0;
const STORY: [(&str, f32, f32); 11] = [
    ("It is the present, the year 1985.", 64.0, 20.0),
    ("A robot from the year 2019 has been sent back in time", 84.0, 20.0),
    ("to eliminate the future resistance leader.  The future people", 104.0, 20.0),
    ("also sent someone back to stop the robot.", 124.0, 20.0),
    ("The Eliminator", 164.0, 24.0),
    ("You must kill the future resistance leader.", 188.0, 20.0),
    ("A red box will appear in the HUD showing their last known location.", 208.0, 20.0),
    ("The Protector", 248.0, 24.0),
    ("You must stop The Eliminator, the only way to destroy it is with grenades.", 272.0, 20.0),
    ("Civilian/The Savior", 312.0, 24.0),
    ("Your goal is to survive, one of you is the future savior of mankind.", 336.0, 20.0),
];
const GROUPS: [(usize, &str, &str, f32); 3] = [(1, "The Eliminator", "The Eliminators", 128.0), (0, "The Savior", "The Saviors", 188.0), (2, "The Protector", "The Protectors", 248.0)];

#[derive(Clone, PartialEq)]
struct Text {
    text: String,
    at: Vec2,
    size: f32,
    colour: [f32; 4],
}

#[derive(Clone, PartialEq)]
struct Row {
    player: usize,
    name: String,
    number: i32,
    voice: Option<Voice>,
    ready: bool,
}

#[derive(Clone, PartialEq, Default)]
struct View {
    texts: Vec<Text>,
    rows: Vec<Row>,
}

#[derive(Component)]
struct EliminatorScreen;

#[derive(Component)]
struct TargetLayer;

#[derive(Resource, Default)]
struct Shown(Option<View>);

pub struct EliminatorPlugin;

impl Plugin for EliminatorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Shown>()
            .add_systems(OnEnter(Screen::InGame), spawn_target_layer)
            .add_systems(Update, (show_eliminator, show_target_box.after(Smoothing::Glide)).run_if(in_state(Screen::InGame)).run_if(resource_exists::<NetGame>))
            .add_systems(OnExit(Screen::InGame), |mut shown: ResMut<Shown>| shown.0 = None);
    }
}

fn spawn_target_layer(mut commands: Commands) {
    let full = Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() };

    commands.spawn((full, TargetLayer, Pickable::IGNORE, DespawnOnExit(Screen::InGame)));
}

fn show_target_box(
    mut commands: Commands,
    net: Res<NetGame>,
    scale: Res<UiScale>,
    camera: Single<(&Camera, &GlobalTransform), With<GameCamera>>,
    layer: Single<(Entity, Option<&Children>), With<TargetLayer>>,
) {
    let (layer, children) = *layer;
    children.into_iter().flatten().for_each(|&c| commands.entity(c).despawn());

    if net.sync.game_type != GameMode::Eliminator {
        return;
    }

    let tint = match net.role {
        ELIMINATOR => BOX_RED,
        PROTECTOR => BOX_GREEN,
        _ => return,
    };

    let Some(target) = net.target.filter(|t| t.x != 0.0) else { return };

    let (camera, view) = *camera;
    let size = (BOX_NEAR / target.distance(view.translation()) + 4.0).clamp(BOX_MIN, BOX_MAX);

    let Ok(on_screen) = camera.world_to_viewport(view, target) else { return };
    let (node, shift) = anchored(align::CENTRE, on_screen / scale.0);

    commands.spawn((
        Node { width: px(size), height: px(size), border: UiRect::all(px(size / 16.0)), ..node },
        shift,
        BorderColor::all(colour(tint)),
        Pickable::IGNORE,
        ChildOf(layer),
    ));
}

fn say(text: impl Into<String>, x: f32, y: f32, size: f32, colour: [f32; 4]) -> Text {
    Text { text: text.into(), at: Vec2::new(x, y), size, colour }
}

fn view(net: &NetGame) -> Option<View> {
    let lobby = net.game_state == GameState::Intermission && net.me().is_some_and(|p| p.menu == MenuType::Lobby);
    let restarting = net.game_state == GameState::Restarting;

    if net.sync.game_type != GameMode::Eliminator || !(lobby || restarting) {
        return None;
    }

    let mut v = View::default();
    let one = net.result_count == 1;

    if restarting {
        let humans_won = net.corp_round[0].0 > 0;
        let headline = if humans_won { "Humans Win!" } else if one { "Eliminator Wins!" } else { "Eliminators Wins!" };
        v.texts.push(say(headline, 256.0, 64.0, 32.0, if humans_won { WHITE } else { RED }));

        for (k, single, many, y) in GROUPS {
            let names: Vec<String> = net.results[..net.result_count].iter().map(|r| usize::try_from(r[k]).ok().and_then(|p| net.players.get(p)).map_or(String::new(), |p| p.name.clone())).collect();

            v.texts.push(say(if one { single } else { many }, 4.0, y, 20.0, PALE));
            v.texts.push(say(names.join(", "), 4.0, y + 20.0, 20.0, PALE));
        }
    }

    if net.game_state == GameState::Intermission {
        v.texts.push(say("THE ELIMINATOR", 256.0, 16.0, 32.0, PALE));
        v.texts.extend(STORY.iter().map(|&(s, y, size)| say(s, 4.0, y, size, PALE)));
        v.texts.push(say("The Players", 776.0, 98.0, 17.5, WHITE));

        v.rows = net
            .players
            .iter()
            .enumerate()
            .filter(|(_, p)| p.active && !p.is_bot)
            .map(|(k, p)| Row { player: k, name: p.name.clone(), number: p.phone_number, voice: (k as i32 != net.player).then(|| Voice::of(p)), ready: p.is_ready })
            .collect();
    }

    Some(v)
}

fn show_eliminator(mut commands: Commands, net: Res<NetGame>, fonts: Res<GameFonts>, icons: Res<VoiceIcons>, mut shown: ResMut<Shown>, screens: Query<&ChildOf, With<EliminatorScreen>>) {
    let now = view(&net);

    if shown.0 == now {
        return;
    }

    screens.iter().for_each(|window| commands.entity(window.parent()).despawn());

    if let Some(v) = &now {
        let screen = spawn_screen(&mut commands, EliminatorScreen, Stick::Centre);
        commands.entity(screen).insert((Visibility::Inherited, GlobalZIndex(ROSTER_LAYER)));

        for t in &v.texts {
            let (node, shift) = anchored(align::TOP_LEFT, t.at);
            commands.spawn((node, shift, text(t.text.clone(), &fonts.rockwell, t.size, colour(t.colour)), ChildOf(screen)));
        }

        for (k, row) in v.rows.iter().enumerate() {
            let y = 128.0 + k as f32 * ROW;
            let dim = if row.ready { 1.0 } else { 0.5 };
            let shade = colour([dim * 0.75, dim * 0.75, dim, 1.0]);

            if let Some(voice) = row.voice {
                let holder = commands.spawn((Node { position_type: PositionType::Absolute, left: px(752.0), top: px(y), ..default() }, Pickable::IGNORE, ChildOf(screen))).id();
                voice_icon(&mut commands, holder, &icons, voice, ICON, Some(row.player));
            }

            let (node, shift) = anchored(align::TOP_LEFT, Vec2::new(776.0, y));
            commands.spawn((node, shift, text(row.name.clone(), &fonts.rockwell, 17.5, shade), ChildOf(screen)));
            let (node, shift) = anchored(align::TOP_LEFT, Vec2::new(776.0, y + 18.0));
            commands.spawn((node, shift, text(phone_number(row.number as u32), &fonts.rockwell, 10.0, shade), ChildOf(screen)));
        }
    }

    shown.0 = now;
}
