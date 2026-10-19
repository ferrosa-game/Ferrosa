use bevy::{
    camera::{Exposure, RenderTarget, ScalingMode},
    core_pipeline::tonemapping::Tonemapping,
    input::mouse::{AccumulatedMouseScroll, MouseScrollUnit},
    prelude::*,
    render::render_resource::TextureFormat,
    ui::RelativeCursorPosition,
};
use ferrosa_protocol::{GameMode, Team};

use super::{
    GAME_SCREEN, Level,
    briefing::{Row, RowKind, briefing_rows},
    camera::EXPOSURE,
    computer::AtComputer,
    escape_menu::{GameMenu, MENU_LAYER, OVERLAY_LAYER},
    hud::{Stick, align, anchored, colour, rect, spawn_screen, text},
    items::ItemTypes,
    netgame::NetGame,
};
use crate::{fonts::GameFonts, menu::Screen, names::{ROUND_CORPORATIONS, TEAM_COLOURS}};

const ROUND_ORIGIN: Vec2 = Vec2::new(960.0, 896.0);
const CITY_ORIGIN: Vec2 = Vec2::new(896.0, 896.0);
const ROUND_SCALE: f32 = 0.25;
const CITY_SCALE: f32 = 0.125;
const SPAN: f32 = 256.0;
const PICTURE_SIZE: u32 = 1024;
const PICTURE_FRAMES: u32 = 8;
const CAMERA_HEIGHT: f32 = 1000.0;

const VIEW_AT: Vec2 = Vec2::new(16.0, 16.0);
const VIEW_SIZE: f32 = 544.0;
const PANEL_AT: Vec2 = Vec2::new(576.0, 16.0);
const PANEL_SIZE: Vec2 = Vec2::new(432.0, 544.0);
const MINI_SIZE: f32 = 176.0;
const MINI_AT: Vec2 = Vec2::new(GAME_SCREEN.x - 16.0 - MINI_SIZE, 36.0);
const MINI_WORLD_SPAN: f32 = 256.0;
const MAX_ZOOM: f32 = 8.0;
const ZOOM_STEP: f32 = 1.2;
const PIXELS_PER_LINE: f32 = 100.0;

const BACKDROP: Color = Color::srgba(0.02, 0.025, 0.03, 0.97);
const PANEL: Color = Color::srgba(0.08, 0.09, 0.11, 0.95);
const FRAME: Color = Color::srgb(0.45, 0.47, 0.5);
const MAP_GROUND: Color = Color::srgb(0.05, 0.05, 0.06);
const HEADING: [f32; 4] = [1.0, 0.85, 0.5, 1.0];
const BODY: [f32; 4] = [0.85, 0.85, 0.85, 1.0];
const HINT: [f32; 4] = [0.6, 0.6, 0.65, 1.0];
const BORDER: f32 = 2.0;
const PADDING: f32 = 12.0;

const RING_WIDTH: f32 = 2.0;
const MARKER_SIZE: f32 = 12.0;
const MARKER_EDGE: f32 = 2.0;
const MARKER_COLOUR: [f32; 4] = [1.0, 0.15, 0.15, 1.0];
const DROP_COLOUR: [f32; 4] = [0.2, 1.0, 0.3, 1.0];
const SPOT_COLOUR: [f32; 4] = [0.75, 0.75, 0.75, 1.0];
const BASE_LABEL: [f32; 4] = [0.95, 0.95, 0.95, 1.0];
const STREET_LABEL: [f32; 4] = [1.0, 1.0, 1.0, 0.85];
const BANK_LABEL: [f32; 4] = [0.3, 0.85, 0.3, 1.0];
const LABEL: f32 = 6.0;
const LABEL_SIZES: (f32, f32) = (10.0, 16.0);
const BANK_SIGN: f32 = 9.0;
const BANK_SIZES: (f32, f32) = (12.0, 22.0);

const BASE: i32 = 1;
const BANK: i32 = 6;
const DROP_MISSION: i32 = 2;
const SIDE_STREETS: i32 = 0x140;
const AVENUE_START: i32 = 0x100;
const AVENUE_BLOCK: i32 = 0x38;
const GRID: f32 = 4.0;
const MISSION_SPOTS: [Vec2; 8] = [
    Vec2::new(1718.0, 1262.0),
    Vec2::new(1778.0, 1006.0),
    Vec2::new(1470.0, 1494.0),
    Vec2::new(1282.0, 1534.0),
    Vec2::new(1674.0, 1510.0),
    Vec2::new(1346.0, 1298.0),
    Vec2::new(1606.0, 1166.0),
    Vec2::new(1546.0, 1278.0),
];

#[derive(Resource)]
struct MapPicture {
    image: Handle<Image>,
    origin: Vec2,
    scale: f32,
    round: bool,
}

impl MapPicture {
    fn on_map(&self, world: Vec2) -> Vec2 {
        (world - self.origin) * self.scale
    }
}

#[derive(Resource, Default)]
struct MinimapOpen(bool);

#[derive(Resource)]
struct MapView {
    zoom: f32,
    offset: Vec2,
    drag: Option<Vec2>,
}

impl Default for MapView {
    fn default() -> Self {
        Self { zoom: fit_zoom(), offset: Vec2::ZERO, drag: None }
    }
}

#[derive(Component)]
struct PictureCamera(u32);

#[derive(Component, Clone, Copy, PartialEq)]
enum Viewport {
    Full,
    Mini,
}

#[derive(Component, Default)]
struct Canvas(Option<(f32, Vec<Mark>)>);

#[derive(Component)]
struct PlayerPointer;

#[derive(Component)]
struct MiniScreen;

#[derive(Component, Default)]
struct BriefingList(Option<Vec<Row>>);

#[derive(Clone, PartialEq)]
struct Mark {
    at: Vec2,
    radius: f32,
    colour: [f32; 4],
}

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MinimapOpen>()
            .init_resource::<MapView>()
            .add_systems(OnEnter(Screen::InGame), (prepare_picture, spawn_minimap).chain())
            .add_systems(OnEnter(GameMenu::Map), spawn_map_screen)
            .add_systems(Update, (toggle_maps, finish_picture).run_if(in_state(Screen::InGame)).run_if(resource_exists::<MapPicture>))
            .add_systems(Update, (steer_map, fill_briefing).run_if(in_state(GameMenu::Map)).run_if(resource_exists::<NetGame>))
            .add_systems(Update, (show_minimap, draw_canvases, place_canvases, place_pointers).chain().run_if(in_state(Screen::InGame)).run_if(resource_exists::<MapPicture>))
            .add_systems(OnExit(Screen::InGame), |mut commands: Commands, mut open: ResMut<MinimapOpen>, mut view: ResMut<MapView>| {
                open.0 = false;
                *view = MapView::default();
                commands.remove_resource::<MapPicture>();
            });
    }
}

fn fit_zoom() -> f32 {
    VIEW_SIZE / SPAN
}

fn prepare_picture(mut commands: Commands, assets: Res<AssetServer>, level: Res<Level>, mut images: ResMut<Assets<Image>>) {
    if level.map.map_name == "round" {
        commands.insert_resource(MapPicture { image: assets.load("round/map.png"), origin: ROUND_ORIGIN, scale: ROUND_SCALE, round: true });
        return;
    }

    let image = images.add(Image::new_target_texture(PICTURE_SIZE, PICTURE_SIZE, TextureFormat::Rgba8UnormSrgb, None));
    let span = SPAN / CITY_SCALE;
    let centre = CITY_ORIGIN + Vec2::splat(span * 0.5);
    let projection = OrthographicProjection { scaling_mode: ScalingMode::Fixed { width: span, height: span }, near: 0.0, far: CAMERA_HEIGHT * 2.0, ..OrthographicProjection::default_3d() };

    commands.spawn((
        PictureCamera(0),
        Camera3d::default(),
        Camera { order: -2, clear_color: ClearColorConfig::Custom(MAP_GROUND), ..default() },
        RenderTarget::Image(image.clone().into()),
        Projection::Orthographic(projection),
        Exposure { ev100: EXPOSURE },
        Tonemapping::AcesFitted,
        Transform::from_xyz(centre.x, CAMERA_HEIGHT, centre.y).looking_at(Vec3::new(centre.x, 0.0, centre.y), Vec3::NEG_Z),
        DespawnOnExit(Screen::InGame),
    ));

    commands.insert_resource(MapPicture { image, origin: CITY_ORIGIN, scale: CITY_SCALE, round: false });
}

fn finish_picture(mut commands: Commands, mut cameras: Query<(Entity, &mut PictureCamera)>) {
    for (e, mut camera) in &mut cameras {
        camera.0 += 1;

        if camera.0 > PICTURE_FRAMES {
            commands.entity(e).despawn();
        }
    }
}

fn viewport(commands: &mut Commands, parent: Entity, at: Vec2, size: f32, kind: Viewport) -> Entity {
    let node = Node { border: UiRect::all(px(BORDER)), overflow: Overflow::clip(), ..rect(align::TOP_LEFT, at, Vec2::splat(size)) };
    let view = commands.spawn((node, BackgroundColor(MAP_GROUND), BorderColor::all(FRAME), kind, RelativeCursorPosition::default(), ChildOf(parent))).id();

    commands.spawn((Node { position_type: PositionType::Absolute, ..default() }, Canvas::default(), Pickable::IGNORE, ChildOf(view)));

    view
}

fn spawn_minimap(mut commands: Commands) {
    let screen = spawn_screen(&mut commands, MiniScreen, Stick::TopRight);
    commands.entity(screen).insert(GlobalZIndex(OVERLAY_LAYER));
    viewport(&mut commands, screen, MINI_AT, MINI_SIZE, Viewport::Mini);
}

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

fn spawn_map_screen(mut commands: Commands, net: Option<Res<NetGame>>, fonts: Res<GameFonts>) {
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

fn fill_briefing(
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

fn toggle_maps(keys: Res<ButtonInput<KeyCode>>, menu: Res<State<GameMenu>>, at_computer: Res<AtComputer>, mut next: ResMut<NextState<GameMenu>>, mut minimap: ResMut<MinimapOpen>) {
    match menu.get() {
        GameMenu::Closed | GameMenu::PlayerMenu if !at_computer.typing() => {
            if keys.just_pressed(KeyCode::KeyC) {
                next.set(GameMenu::Map);
            }

            if keys.just_pressed(KeyCode::KeyM) {
                minimap.0 = !minimap.0;
            }
        }
        GameMenu::Map if keys.any_just_pressed([KeyCode::KeyC, KeyCode::Escape]) => next.set(GameMenu::Closed),
        _ => {}
    }
}

fn player_on_map(net: &NetGame, picture: &MapPicture) -> Option<Vec2> {
    let p = net.me()?;
    let h = usize::try_from(p.human).ok().and_then(|h| net.humans.get(h)).filter(|h| h.active)?;

    Some(picture.on_map(Vec2::new(h.track.pos.x, h.track.pos.z)))
}

fn clamp_offset(offset: Vec2, zoom: f32) -> Vec2 {
    let extent = SPAN * zoom;
    let axis = |o: f32| if extent <= VIEW_SIZE { (VIEW_SIZE - extent) * 0.5 } else { o.clamp(VIEW_SIZE - extent, 0.0) };

    Vec2::new(axis(offset.x), axis(offset.y))
}

fn steer_map(
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

fn show_minimap(minimap: Res<MinimapOpen>, menu: Res<State<GameMenu>>, net: Option<Res<NetGame>>, mut screens: Query<&mut Visibility, With<MiniScreen>>) {
    let shown = minimap.0 && *menu.get() != GameMenu::Map && net.is_some_and(|n| n.me().is_some());

    for mut v in &mut screens {
        v.set_if_neq(if shown { Visibility::Inherited } else { Visibility::Hidden });
    }
}

fn marks(net: &NetGame, picture: &MapPicture, level: &Level) -> Vec<Mark> {
    let mut out = Vec::new();
    let me = net.me().map(|p| p.team);
    let team_colour = |i: usize| {
        let [r, g, b] = TEAM_COLOURS[i];
        [r, g, b, 1.0]
    };
    let base = |i: usize| level.map.level.bases.get(i).map(|b| picture.on_map(Vec2::new(b.interior_min.x + b.interior_max.x, b.interior_min.z + b.interior_max.z) * 0.5));
    let mine = |i: usize| me == Team::CORPORATIONS.get(i).copied();

    match net.sync.game_type {
        GameMode::World => {
            for i in 0..Team::CORPORATIONS.len() {
                let Some(at) = base(i) else { continue };

                out.push(Mark { at, radius: 16.0, colour: team_colour(i) });

                if mine(i) {
                    out.push(Mark { at, radius: 8.0, colour: team_colour(i) });
                }

                if net.players.iter().any(|p| p.active && p.team == Team::CORPORATIONS[i]) {
                    out.push(Mark { at, radius: 12.0, colour: team_colour(i) });
                }
            }
        }
        mode @ (GameMode::Round | GameMode::Eliminator) => {
            for i in 0..ROUND_CORPORATIONS.len() {
                let Some(at) = base(i) else { continue };

                out.push(Mark { at, radius: 16.0, colour: if mode == GameMode::Round { team_colour(i) } else { [1.0; 4] } });

                if mine(i) {
                    out.push(Mark { at, radius: 14.0, colour: team_colour(i) });
                }
            }
        }
        _ => {}
    }

    if net.sync.game_type == GameMode::Round
        && let Some(missions) = me.and_then(|t| Team::CORPORATIONS.iter().position(|&c| c == t)).and_then(|t| net.missions.get(t))
    {
        for m in missions.iter().filter(|m| m.active && m.kind == DROP_MISSION) {
            if let Some(&spot) = usize::try_from(m.location).ok().and_then(|k| MISSION_SPOTS.get(k)) {
                out.push(Mark { at: picture.on_map(spot), radius: 8.0, colour: DROP_COLOUR });
            }
        }
    }

    if net.sync.game_type == GameMode::Eliminator {
        out.extend(MISSION_SPOTS.iter().map(|&spot| Mark { at: picture.on_map(spot), radius: 8.0, colour: SPOT_COLOUR }));
    }

    out
}

fn label_size(base: f32, zoom: f32, (low, high): (f32, f32)) -> f32 {
    (base * zoom).clamp(low, high)
}

fn city_labels(commands: &mut Commands, canvas: Entity, picture: &MapPicture, level: &Level, font: &Handle<Font>, zoom: f32) {
    let streets = &level.map.streets;
    let mut label = |s: String, at: Vec2, size: f32, shade: [f32; 4]| {
        let (node, shift) = anchored(align::CENTRE, at * zoom);
        commands.spawn((node, shift, text(s, font, size, colour(shade)), TextShadow { offset: Vec2::splat(1.0), color: Color::BLACK }, ChildOf(canvas)));
    };

    for s in &streets.streets {
        let (Some(a), Some(b)) = (streets.intersections.get(s.intersections[0]), streets.intersections.get(s.intersections[1])) else { continue };
        let shown = match s.north_south {
            0 => a.streets[2] == -1,
            1 => (a.pos.z - SIDE_STREETS) as u32 <= 3,
            _ => true,
        };

        if !shown {
            continue;
        }

        let middle = Vec2::new((a.pos.x + b.pos.x) as f32, (a.pos.z + b.pos.z) as f32) * 0.5 * GRID;
        let stagger = if s.north_south == 1 { (((a.pos.x - AVENUE_START) / AVENUE_BLOCK) & 1) as f32 * 16.0 } else { 0.0 };

        label(s.name.to_string(), picture.on_map(middle) + Vec2::Y * stagger, label_size(LABEL, zoom, LABEL_SIZES), STREET_LABEL);
    }

    for b in &level.map.level.buildings {
        let at = picture.on_map(Vec2::new(b.interior_min.x + b.interior_max.x, b.interior_min.z + b.interior_max.z) * 0.5);

        match b.kind {
            BASE => {
                let name = usize::try_from(b.team).ok().and_then(|t| Team::CORPORATION_NAMES.get(t)).map_or_else(|| format!("team {}", b.team + 1), |n| n.to_string());
                label(name, at, label_size(LABEL, zoom, LABEL_SIZES), BASE_LABEL);
            }
            BANK => label("$".to_string(), at, label_size(BANK_SIGN, zoom, BANK_SIZES), BANK_LABEL),
            _ => {}
        }
    }
}

fn canvas_zoom(kind: Viewport, view: &MapView, picture: &MapPicture) -> f32 {
    match kind {
        Viewport::Full => view.zoom,
        Viewport::Mini => MINI_SIZE / (MINI_WORLD_SPAN * picture.scale),
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_canvases(
    mut commands: Commands,
    net: Option<Res<NetGame>>,
    picture: Res<MapPicture>,
    level: Res<Level>,
    view: Res<MapView>,
    fonts: Res<GameFonts>,
    mut canvases: Query<(Entity, &mut Canvas, &ChildOf)>,
    viewports: Query<&Viewport>,
) {
    let Some(net) = net else { return };
    let wanted = marks(&net, &picture, &level);

    for (e, mut canvas, ChildOf(parent)) in &mut canvases {
        let Ok(&kind) = viewports.get(*parent) else { continue };
        let zoom = canvas_zoom(kind, &view, &picture);

        if canvas.0.as_ref().is_some_and(|(z, m)| *z == zoom && *m == wanted) {
            continue;
        }

        commands.entity(e).despawn_children();
        commands.spawn((rect(align::TOP_LEFT, Vec2::ZERO, Vec2::splat(SPAN * zoom)), ImageNode::new(picture.image.clone()), Pickable::IGNORE, ChildOf(e)));

        if !picture.round {
            city_labels(&mut commands, e, &picture, &level, &fonts.rockwell, zoom);
        }

        for m in &wanted {
            let ring = Node { border: UiRect::all(px(RING_WIDTH)), border_radius: BorderRadius::all(percent(50)), ..rect(align::CENTRE, m.at * zoom, Vec2::splat(m.radius * 2.0 * zoom.min(fit_zoom() * 2.0))) };
            commands.spawn((ring, BorderColor::all(colour(m.colour)), Pickable::IGNORE, ChildOf(e)));
        }

        commands.spawn((marker(Vec2::ZERO), PlayerPointer, BackgroundColor(colour(MARKER_COLOUR)), BorderColor::all(Color::BLACK), Visibility::Hidden, Pickable::IGNORE, ChildOf(e)));

        canvas.0 = Some((zoom, wanted.clone()));
    }
}

fn place_canvases(net: Option<Res<NetGame>>, picture: Res<MapPicture>, view: Res<MapView>, mut canvases: Query<(&mut Node, &ChildOf), With<Canvas>>, viewports: Query<&Viewport>) {
    let player = net.and_then(|n| player_on_map(&n, &picture));

    for (mut node, ChildOf(parent)) in &mut canvases {
        let Ok(&kind) = viewports.get(*parent) else { continue };
        let offset = match kind {
            Viewport::Full => view.offset,
            Viewport::Mini => {
                let zoom = canvas_zoom(kind, &view, &picture);
                Vec2::splat(MINI_SIZE * 0.5) - player.unwrap_or(Vec2::splat(SPAN * 0.5)) * zoom
            }
        };

        if node.left != px(offset.x) || node.top != px(offset.y) {
            node.left = px(offset.x);
            node.top = px(offset.y);
        }
    }
}

fn marker(centre: Vec2) -> Node {
    Node { border: UiRect::all(px(MARKER_EDGE)), border_radius: BorderRadius::all(percent(50)), ..rect(align::CENTRE, centre, Vec2::splat(MARKER_SIZE)) }
}

fn place_pointers(net: Option<Res<NetGame>>, picture: Res<MapPicture>, canvases: Query<&Canvas>, mut pointers: Query<(&ChildOf, &mut Node, &mut Visibility), With<PlayerPointer>>) {
    let player = net.and_then(|n| player_on_map(&n, &picture));

    for (ChildOf(canvas), mut node, mut visibility) in &mut pointers {
        let (Some(at), Some(zoom)) = (player, canvases.get(*canvas).ok().and_then(|c| c.0.as_ref().map(|(z, _)| *z))) else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        let placed = marker(at * zoom);

        if node.left != placed.left || node.top != placed.top {
            *node = placed;
        }

        visibility.set_if_neq(Visibility::Inherited);
    }
}
