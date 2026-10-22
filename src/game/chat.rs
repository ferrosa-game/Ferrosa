use bevy::{
    input::keyboard::KeyboardInput,
    input_focus::{FocusCause, FocusedInput, InputFocus},
    prelude::*,
    text::EditableText,
    ui_widgets::TextInput,
};
use ferrosa_protocol::{
    GameMode,
    clientbound::game::events::chat::ChatType,
    serverbound::game::actions::{ChatAction, GameAction},
};
use ferrosa_world::skeleton::BoneId;

use super::{
    GAME_FPS,
    camera::GameCamera,
    computer::AtComputer,
    escape_menu::{GameMenu, OVERLAY_LAYER},
    first_person::FirstPerson,
    hud::{HudScreen, Stick, align, anchored, colour, spawn_screen, text},
    human_sync::{NetHuman, bone_at},
    netgame::{ChatLine, NetGame},
    smoothing::Smoothing,
};
use crate::{fonts::GameFonts, character_model::HumanPose, menu::Screen};

const LOG_AT: Vec2 = Vec2::new(4.0, 400.0);
const BACKING: Color = Color::srgba(0.15, 0.15, 0.15, 0.6);
const ANNOUNCE_AT: Vec2 = Vec2::new(512.0, 80.0);
const ANNOUNCE_COLOUR: [f32; 3] = [0.75, 0.75, 1.0];
const LABEL_AT: Vec2 = Vec2::new(4.0, 320.0);
const ENTRY_AT: Vec2 = Vec2::new(60.0, 320.0);
const ENTRY_SHADE: Color = Color::srgba(0.0, 0.0, 0.0, 0.5);
const LEVEL_AT: Vec2 = Vec2::new(4.0, 524.0);
const LEVELS: [&str; 3] = ["Whisper", "Say", "Yell"];

#[derive(Resource)]
pub struct ChatState {
    pub level: i32,
}

impl Default for ChatState {
    fn default() -> Self {
        Self { level: 1 }
    }
}

#[derive(Component)]
struct ChatScreen;

#[derive(Component)]
struct ChatLog;

#[derive(Component)]
struct Announcements;

#[derive(Component)]
struct LevelText;

#[derive(Component)]
struct Bubbles;

#[derive(Component)]
struct Entry;

#[derive(Component)]
struct Faded(f32);

#[derive(Clone, PartialEq)]
struct Shown {
    text: String,
    colour: [f32; 3],
}

pub struct ChatPlugin;

impl Plugin for ChatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChatState>()
            .add_systems(OnEnter(Screen::InGame), spawn_chat)
            .add_systems(OnEnter(GameMenu::Chat), open_entry)
            .add_systems(Update, open_chat.run_if(in_state(GameMenu::Closed)))
            .add_systems(
                Update,
                (age_chat, show_log, show_announcements, show_level, fade, show_bubbles.after(Smoothing::Glide)).chain().run_if(in_state(Screen::InGame)).run_if(resource_exists::<NetGame>),
            );
    }
}

fn spawn_chat(mut commands: Commands, fonts: Res<GameFonts>) {
    let screen = spawn_screen(&mut commands, ChatScreen, Stick::BottomLeft);
    commands.entity(screen).insert((Visibility::Inherited, GlobalZIndex(OVERLAY_LAYER)));

    let (node, shift) = anchored(align::TOP_LEFT, LOG_AT);
    commands.spawn((Node { flex_direction: FlexDirection::Column, align_items: AlignItems::FlexStart, row_gap: px(2), ..node }, shift, ChatLog, Pickable::IGNORE, ChildOf(screen)));

    let top = spawn_screen(&mut commands, ChatScreen, Stick::Top);
    commands.entity(top).insert((Visibility::Inherited, GlobalZIndex(OVERLAY_LAYER)));

    let (node, shift) = anchored(align::CENTRE_X, ANNOUNCE_AT);
    commands.spawn((Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..node }, shift, Announcements, Pickable::IGNORE, ChildOf(top)));

    let hud = spawn_screen(&mut commands, HudScreen, Stick::BottomLeft);
    let (node, shift) = anchored(align::BOTTOM, LEVEL_AT);
    commands.spawn((node, shift, LevelText, text("", &fonts.rockwell, 18.0, Color::WHITE), ChildOf(hud)));

    let full = Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() };
    commands.spawn((full, Bubbles, Pickable::IGNORE, DespawnOnExit(Screen::InGame)));
}

fn age_chat(time: Res<Time>, mut net: ResMut<NetGame>) {
    let dt = time.delta_secs();

    for line in net.chat.iter_mut().filter(|l| l.life > 0.0) {
        line.life -= dt;
    }
}

fn alpha(life: f32) -> f32 {
    (life * GAME_FPS / 200.0).clamp(0.0, 1.0)
}

fn rebuild(commands: &mut Commands, panel: Entity, children: Option<&Children>, shown: &[(Shown, f32)], font: &Handle<Font>, size: f32) {
    children.into_iter().flatten().for_each(|&c| commands.entity(c).despawn());

    for (line, life) in shown {
        let [r, g, b] = line.colour;
        let backing = BackgroundColor(BACKING.with_alpha(BACKING.alpha() * alpha(*life)));
        let node = Node { padding: UiRect::axes(px(4), px(1)), ..default() };

        commands.spawn((text(line.text.clone(), font, size, Color::srgba(r, g, b, alpha(*life))), node, backing, Faded(*life), ChildOf(panel)));
    }
}

fn log_lines(net: &NetGame) -> Vec<(Shown, f32)> {
    net.chat
        .iter()
        .filter(|l| l.life > 0.0 && !l.text.is_empty())
        .filter(|l| match l.kind {
            ChatType::Announce | ChatType::AdminChat | ChatType::Chat | ChatType::ItemSpeak => true,
            ChatType::PrivateMessage => l.speaker == net.player,
            _ => false,
        })
        .map(|l| {
            let player = match l.kind {
                ChatType::Announce => Some(l.speaker),
                ChatType::Chat => usize::try_from(l.speaker).ok().and_then(|h| net.humans.get(h)).map(|h| h.player),
                _ => None,
            };
            let name = player.and_then(|p| usize::try_from(p).ok()).and_then(|p| net.players.get(p)).filter(|p| p.active).map(|p| p.name.as_str());
            let name = if l.kind == ChatType::ItemSpeak { Some("Phone") } else { name };
            let text = name.map_or_else(|| l.text.clone(), |n| format!("{n}: {}", l.text));
            let colour = match (l.kind, l.volume) {
                (ChatType::PrivateMessage, _) => [1.0, 0.0, 0.0],
                (ChatType::ItemSpeak, _) => [0.75; 3],
                (ChatType::Chat, 0) => [0.6, 0.6, 1.0],
                (ChatType::Chat, 2) => [1.0, 0.6, 0.6],
                _ => [1.0; 3],
            };

            (Shown { text, colour }, l.life)
        })
        .collect()
}

fn same(a: &[(Shown, f32)], b: &[Shown]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|((x, _), y)| x == y)
}

fn show_log(
    mut commands: Commands,
    net: Res<NetGame>,
    fonts: Res<GameFonts>,
    panel: Single<(Entity, Option<&Children>), With<ChatLog>>,
    mut last: Local<Vec<Shown>>,
    mut lines: Query<&mut Faded>,
) {
    let shown = log_lines(&net);
    let (panel, children) = *panel;

    if same(&shown, &last) {
        for (&c, (_, life)) in children.into_iter().flatten().zip(&shown) {
            if let Ok(mut faded) = lines.get_mut(c) {
                faded.0 = *life;
            }
        }
        return;
    }

    rebuild(&mut commands, panel, children, &shown, &fonts.rockwell, 20.0);
    *last = shown.into_iter().map(|(s, _)| s).collect();
}

fn show_announcements(
    mut commands: Commands,
    net: Res<NetGame>,
    fonts: Res<GameFonts>,
    panel: Single<(Entity, Option<&Children>), With<Announcements>>,
    mut last: Local<Vec<Shown>>,
    mut lines: Query<&mut Faded>,
) {
    let eliminator = net.sync.game_type == GameMode::Eliminator;
    let shown: Vec<(Shown, f32)> = net
        .chat
        .iter()
        .filter(|l| eliminator && l.kind == ChatType::EliminatorAnnouncement && l.life > 0.0 && !l.text.is_empty())
        .map(|l| (Shown { text: l.text.clone(), colour: ANNOUNCE_COLOUR }, l.life))
        .collect();
    let (panel, children) = *panel;

    if same(&shown, &last) {
        for (&c, (_, life)) in children.into_iter().flatten().zip(&shown) {
            if let Ok(mut faded) = lines.get_mut(c) {
                faded.0 = *life;
            }
        }
        return;
    }

    rebuild(&mut commands, panel, children, &shown, &fonts.rockwell, 24.0);
    *last = shown.into_iter().map(|(s, _)| s).collect();
}

fn fade(mut lines: Query<(&Faded, &mut TextColor, &mut BackgroundColor), Changed<Faded>>) {
    for (faded, mut colour, mut backing) in &mut lines {
        colour.0.set_alpha(alpha(faded.0));
        backing.0.set_alpha(BACKING.alpha() * alpha(faded.0));
    }
}

fn show_level(state: Res<ChatState>, mut level: Single<(&mut Text, &mut Visibility), With<LevelText>>) {
    let (text, visibility) = &mut *level;
    let word = (state.level != 1).then(|| LEVELS[state.level as usize]);

    visibility.set_if_neq(if word.is_some() { Visibility::Inherited } else { Visibility::Hidden });

    if let Some(word) = word
        && text.0 != word
    {
        text.0 = word.to_string();
    }
}

fn open_chat(keys: Res<ButtonInput<KeyCode>>, at_computer: Res<AtComputer>, mut state: ResMut<ChatState>, mut next: ResMut<NextState<GameMenu>>) {
    if at_computer.typing() {
        return;
    }

    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
        next.set(GameMenu::Chat);
    }

    if keys.just_pressed(KeyCode::KeyT) {
        state.level = (state.level + 1) % LEVELS.len() as i32;
    }
}

fn open_entry(mut commands: Commands, net: Option<Res<NetGame>>, state: Res<ChatState>, mut focus: ResMut<InputFocus>) {
    let screen = spawn_screen(&mut commands, DespawnOnExit(GameMenu::Chat), Stick::BottomLeft);
    commands.entity(screen).insert((Visibility::Inherited, GlobalZIndex(OVERLAY_LAYER)));

    let playing = net.is_some_and(|n| n.me().is_some_and(|p| p.human >= 0));
    let label = if playing { format!("{}:", LEVELS[state.level as usize]) } else { "Chat:".to_string() };
    let (node, shift) = anchored(align::TOP_LEFT, LABEL_AT);
    commands.spawn((node, shift, Text::new(label), TextFont::from_font_size(16.0), TextColor(Color::WHITE), ChildOf(screen)));

    let mut editable = EditableText::new("");
    editable.max_characters = Some(63);

    let (node, _) = anchored(align::TOP_LEFT, ENTRY_AT);
    let entry = commands
        .spawn((Node { width: px(600.0), ..node }, Entry, TextInput, editable, TextFont::from_font_size(16.0), BackgroundColor(ENTRY_SHADE), ChildOf(screen)))
        .observe(finish_entry)
        .id();

    focus.set(entry, FocusCause::Navigated);
}

fn finish_entry(
    mut key: On<FocusedInput<KeyboardInput>>,
    texts: Query<&EditableText, With<Entry>>,
    state: Res<ChatState>,
    mut net: ResMut<NetGame>,
    mut focus: ResMut<InputFocus>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut next: ResMut<NextState<GameMenu>>,
) {
    let input = &key.event().input;

    if !input.state.is_pressed() {
        return;
    }

    let pressed = input.key_code;
    let send = matches!(pressed, KeyCode::Enter | KeyCode::NumpadEnter);

    if !send && pressed != KeyCode::Escape {
        return;
    }

    key.propagate(false);

    let message = texts.get(key.focused_entity).map(|t| t.value().to_string().trim().to_string()).unwrap_or_default();
    let me = usize::try_from(net.player).ok();

    if let Some(p) = me.and_then(|me| net.players.get_mut(me)).filter(|_| send && !message.is_empty()) {
        p.queue(GameAction::Chat(ChatAction { message, volume: state.level as u8 }));
    }

    keys.clear_just_pressed(pressed);
    focus.clear();
    next.set(GameMenu::Closed);
}

fn bubble_colour(line: &ChatLine) -> [f32; 3] {
    let shade = if line.kind == ChatType::ItemSpeak { 0.5 } else { 0.0 };

    match line.volume {
        0 => [shade, shade, shade + 0.5],
        2 => [shade + 0.5, shade, shade],
        _ => [shade; 3],
    }
}

fn bubble_size(line: &ChatLine, distance: f32) -> f32 {
    let size = 32.0 / (distance * 0.125);
    let loud = match line.volume {
        0 => size * 0.25,
        2 => size * 2.0,
        _ => size,
    };

    if line.kind == ChatType::ItemSpeak { loud * 0.125 } else { loud }
}

fn show_bubbles(
    mut commands: Commands,
    net: Res<NetGame>,
    fonts: Res<GameFonts>,
    scale: Res<UiScale>,
    camera: Single<(&Camera, &GlobalTransform), With<GameCamera>>,
    humans: Query<(&NetHuman, &Transform, &HumanPose)>,
    layer: Single<(Entity, Option<&Children>), With<Bubbles>>,
    first_person: Res<FirstPerson>,
) {
    let (layer, children) = *layer;
    children.into_iter().flatten().for_each(|&c| commands.entity(c).despawn());

    let (camera, view) = *camera;
    let up = view.up().as_vec3();
    let mut stacks = std::collections::HashMap::<(bool, i32), f32>::new();

    let own = net.me().map(|p| p.human).filter(|_| first_person.active);

    for line in net.chat.iter().rev().filter(|l| l.life > 0.0 && !l.text.is_empty() && l.speaker >= 0) {
        if line.kind == ChatType::Chat && Some(line.speaker) == own {
            continue;
        }

        let from = match line.kind {
            ChatType::Chat => humans.iter().find(|(nh, ..)| nh.0 as i32 == line.speaker).map(|(_, at, pose)| bone_at(at.translation, pose, BoneId::Head.index()).0),
            ChatType::ItemSpeak => net.items.get(line.speaker as usize).filter(|i| i.active).map(|i| i.track.pos),
            _ => None,
        };
        let Some(from) = from else { continue };

        let at = from + up * 0.5;
        let size = bubble_size(line, at.distance(view.translation())).min(32.0);

        if size <= 4.0 {
            continue;
        }

        let Ok(on_screen) = camera.world_to_viewport(view, at) else { continue };
        let stack = stacks.entry((line.kind == ChatType::ItemSpeak, line.speaker)).or_default();
        let fading = if size < 8.0 { (size - 4.0) / (8.0 - 4.0) } else { 1.0 };
        let a = alpha(line.life) * fading;
        let [r, g, b] = bubble_colour(line);
        let place = on_screen / scale.0 - Vec2::Y * *stack;

        let (node, shift) = anchored(align::CENTRE, place);
        commands.spawn((
            Node { padding: UiRect::horizontal(px(2)), ..node },
            shift,
            BackgroundColor(colour([1.0, 1.0, 1.0, a])),
            text(line.text.clone(), &fonts.rockwell, size * 0.875, Color::srgba(r, g, b, a)),
            ChildOf(layer),
        ));

        *stack += size;
    }
}
