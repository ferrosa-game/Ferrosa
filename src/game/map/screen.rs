use bevy::{
    input::mouse::{AccumulatedMouseScroll, MouseScrollUnit},
    prelude::*,
    ui::RelativeCursorPosition,
};
use ferrosa_protocol::{GameMode, Team};

use super::{
    BANK_LABEL, BORDER, DROP_COLOUR, FRAME, MARKER_COLOUR, MARKER_EDGE, MARKER_SIZE, MapPicture, MapView, RING_WIDTH, SPAN, SPOT_COLOUR, VIEW_SIZE, Viewport, fit_zoom,
    player_on_map, viewport,
};
use crate::{
    fonts::GameFonts,
    game::{
        GAME_SCREEN, Level,
        briefing::{Row, RowKind, briefing_rows},
        escape_menu::{GameMenu, MENU_LAYER},
        hud::{align, colour, rect, text},
        items::ItemTypes,
        netgame::NetGame,
    },
    menu::Screen,
    names::{ROUND_CORPORATIONS, TEAM_COLOURS},
};

const VIEW_AT: Vec2 = Vec2::new(16.0, 16.0);
const PANEL_AT: Vec2 = Vec2::new(576.0, 16.0);
const PANEL_SIZE: Vec2 = Vec2::new(432.0, 544.0);
const MAX_ZOOM: f32 = 8.0;
const ZOOM_STEP: f32 = 1.2;
const PIXELS_PER_LINE: f32 = 100.0;
const BACKDROP: Color = Color::srgba(0.02, 0.025, 0.03, 0.97);
const PANEL: Color = Color::srgba(0.08, 0.09, 0.11, 0.95);
const HEADING: [f32; 4] = [1.0, 0.85, 0.5, 1.0];
const BODY: [f32; 4] = [0.85, 0.85, 0.85, 1.0];
const HINT: [f32; 4] = [0.6, 0.6, 0.65, 1.0];
const PADDING: f32 = 12.0;

#[derive(Component, Default)]
pub(super) struct BriefingList(Option<Vec<Row>>);

fn legend(net: &NetGame) -> Vec<(Option<[f32; 4]>, String)> {
    let mut rows = vec![(None, "You".to_string())];
    let team = net.me().map(|p| p.team);

    match net.sync.game_type {
        GameMode::World => {
            rows.push((Some([1.0; 4]), "Corporation headquarters (team colour)".to_string()));
            rows.push((Some([1.0; 4]), "Middle ring: has staff   Inner ring: yours".to_string()));
            rows.push((Some(BANK_LABEL), "$ Bank".to_string()));
        }
        GameMode::Eliminator => rows.push((Some(SPOT_COLOUR), "Mission spot".to_string())),
        _ => {
            for (k, name) in Team::CORPORATION_NAMES.iter().take(ROUND_CORPORATIONS.len()).enumerate() {
                let [r, g, b] = TEAM_COLOURS[k];
                let own = team == Team::CORPORATIONS.get(k).copied();
                rows.push((Some([r, g, b, 1.0]), if own { format!("{name} base (yours)") } else { format!("{name} base") }));
            }

            rows.push((Some(DROP_COLOUR), "Dead drop".to_string()));
        }
    }

    rows
}

pub(super) fn spawn_map_screen(mut commands: Commands, net: Option<Res<NetGame>>, fonts: Res<GameFonts>) {
    let full = Node { width: percent(100), height: percent(100), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() };
    let backdrop = commands.spawn((full, BackgroundColor(BACKDROP), GlobalZIndex(MENU_LAYER), DespawnOnExit(GameMenu::Map), DespawnOnExit(Screen::InGame))).id();
    let screen = commands.spawn((Node { width: px(GAME_SCREEN.x), height: px(GAME_SCREEN.y), ..default() }, Pickable::IGNORE, ChildOf(backdrop))).id();

    viewport(&mut commands, screen, VIEW_AT, VIEW_SIZE, Viewport::Full);

    let panel = Node {
        flex_direction: FlexDirection::Column,
        padding: UiRect::all(px(PADDING)),
        border: UiRect::all(px(BORDER)),
        row_gap: px(2),
        overflow: Overflow::clip(),
        ..rect(align::TOP_LEFT, PANEL_AT, PANEL_SIZE)
    };
    let panel = commands.spawn((panel, BackgroundColor(PANEL), BorderColor::all(FRAME), Pickable::IGNORE, ChildOf(screen))).id();
    let font = &fonts.rockwell;

    commands.spawn((Node { flex_direction: FlexDirection::Column, row_gap: px(2), flex_grow: 1.0, ..default() }, BriefingList::default(), Pickable::IGNORE, ChildOf(panel)));

    commands.spawn((text("Legend", font, 16.0, colour(HEADING)), ChildOf(panel)));

    for (swatch, label) in net.as_ref().map(|n| legend(n)).unwrap_or_default() {
        let row = commands.spawn((Node { align_items: AlignItems::Center, column_gap: px(8), ..default() }, Pickable::IGNORE, ChildOf(panel))).id();
        let sample = match swatch {
            Some(c) => (Node { width: px(14), height: px(14), border: UiRect::all(px(RING_WIDTH)), border_radius: BorderRadius::all(percent(50)), ..default() }, BorderColor::all(colour(c)), BackgroundColor(Color::NONE)),
            None => (Node { width: px(MARKER_SIZE), height: px(MARKER_SIZE), flex_shrink: 0.0, border: UiRect::all(px(MARKER_EDGE)), border_radius: BorderRadius::all(percent(50)), ..default() }, BorderColor::all(Color::BLACK), BackgroundColor(colour(MARKER_COLOUR))),
        };

        commands.spawn((sample, Pickable::IGNORE, ChildOf(row)));
        commands.spawn((text(label, font, 13.0, colour(BODY)), ChildOf(row)));
    }

    commands.spawn((Node { height: px(8), ..default() }, ChildOf(panel)));
    commands.spawn((text("Wheel: zoom   Drag: pan   Space: find me   C / Esc: close", font, 12.0, colour(HINT)), ChildOf(panel)));
}

pub(super) fn fill_briefing(
    mut commands: Commands,
    net: Res<NetGame>,
    level: Option<Res<Level>>,
    types: Option<Res<ItemTypes>>,
    fonts: Res<GameFonts>,
    mut lists: Query<(Entity, &mut BriefingList)>,
) {
    let rows = briefing_rows(&net, level.as_deref(), types.as_deref());

    for (e, mut list) in &mut lists {
        if list.0.as_ref() == Some(&rows) {
            continue;
        }

        commands.entity(e).despawn_children();

        for r in &rows {
            let (size, shade) = match r.kind {
                RowKind::Title => (22.0, r.colour),
                RowKind::Heading => (16.0, HEADING),
                RowKind::Body => (13.0, BODY),
                RowKind::Blank => {
                    commands.spawn((Node { height: px(6), ..default() }, ChildOf(e)));
                    continue;
                }
            };

            commands.spawn((text(r.text.clone(), &fonts.rockwell, size, colour(shade)), ChildOf(e)));
        }

        list.0 = Some(rows.clone());
    }
}

fn clamp_offset(offset: Vec2, zoom: f32) -> Vec2 {
    let extent = SPAN * zoom;
    let axis = |o: f32| if extent <= VIEW_SIZE { (VIEW_SIZE - extent) * 0.5 } else { o.clamp(VIEW_SIZE - extent, 0.0) };

    Vec2::new(axis(offset.x), axis(offset.y))
}

pub(super) fn steer_map(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    scroll: Res<AccumulatedMouseScroll>,
    net: Res<NetGame>,
    picture: Option<Res<MapPicture>>,
    mut view: ResMut<MapView>,
    viewports: Query<(&Viewport, &RelativeCursorPosition)>,
) {
    let Some(picture) = picture else { return };
    let Some(cursor) = viewports.iter().find(|(v, _)| **v == Viewport::Full).and_then(|(_, c)| c.normalized.filter(|_| c.cursor_over)).map(|n| (n + Vec2::splat(0.5)) * VIEW_SIZE) else {
        view.drag = None;
        return;
    };

    let steps = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / PIXELS_PER_LINE,
    };

    if steps != 0.0 {
        let zoom = (view.zoom * ZOOM_STEP.powf(steps)).clamp(fit_zoom(), fit_zoom() * MAX_ZOOM);
        view.offset = cursor - (cursor - view.offset) * (zoom / view.zoom);
        view.zoom = zoom;
    }

    if mouse.pressed(MouseButton::Left) {
        if let Some(last) = view.drag {
            view.offset += cursor - last;
        }

        view.drag = Some(cursor);
    } else {
        view.drag = None;
    }

    if keys.just_pressed(KeyCode::Space)
        && let Some(at) = player_on_map(&net, &picture)
    {
        view.offset = Vec2::splat(VIEW_SIZE * 0.5) - at * view.zoom;
    }

    view.offset = clamp_offset(view.offset, view.zoom);
}
