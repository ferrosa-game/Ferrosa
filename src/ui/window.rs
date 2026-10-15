use bevy::{prelude::*, ui_widgets::ScrollArea};

use crate::ui::{TEXT_SIZE, text_bundle};

#[derive(Resource, Default)]
pub struct WindowOrder(i32);

#[derive(Component)]
pub struct UiWindow;

#[derive(Component, Clone, Copy)]
enum WindowHandle {
    Title(Entity),
    Resize(Entity),
}

const WINDOW: Color = Color::srgba(0.06, 0.08, 0.1, 0.95);
const WINDOW_TITLE: Color = Color::srgb(0.16, 0.2, 0.25);
const WINDOW_EDGE: Color = Color::srgb(0.3, 0.36, 0.42);

pub fn window(commands: &mut Commands, parent: Entity, title: &str, pos: Vec2, size: Vec2) -> Entity {
    let window = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(pos.x),
                top: px(pos.y),
                width: px(size.x),
                height: px(size.y),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(WINDOW),
            BorderColor::all(WINDOW_EDGE),
            GlobalZIndex(0),
            UiWindow,
            ChildOf(parent),
        ))
        .observe(bring_to_front)
        .id();

    let bar = commands
        .spawn((
            Node { height: px(24), padding: UiRect::horizontal(px(6)), align_items: AlignItems::Center, flex_shrink: 0.0, ..default() },
            BackgroundColor(WINDOW_TITLE),
            WindowHandle::Title(window),
            ChildOf(window),
        ))
        .observe(drag_window)
        .id();

    commands.spawn((text_bundle(title, TEXT_SIZE, Color::WHITE), ChildOf(bar)));

    let contents = commands
        .spawn((
            Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, padding: UiRect::all(px(6)), row_gap: px(4), overflow: Overflow::scroll_y(), ..default() },
            ScrollArea,
            ChildOf(window),
        ))
        .id();

    commands
        .spawn((
            Node { position_type: PositionType::Absolute, right: px(0), bottom: px(0), width: px(12), height: px(12), ..default() },
            BackgroundColor(WINDOW_EDGE),
            WindowHandle::Resize(window),
            ChildOf(window),
        ))
        .observe(drag_window);

    contents
}

fn bring_to_front(press: On<PointerPress>, mut order: ResMut<WindowOrder>, mut windows: Query<&mut GlobalZIndex, With<UiWindow>>) {
    if let Ok(mut z) = windows.get_mut(press.entity) {
        order.0 += 1;
        z.0 = order.0;
    }
}

fn drag_window(mut drag: On<PointerDrag>, handles: Query<&WindowHandle>, mut windows: Query<&mut Node, With<UiWindow>>) {
    let Ok(&handle) = handles.get(drag.entity) else { return };

    drag.propagate(false);

    let nudge = |v: &mut Val, d: f32, least: f32| {
        if let Val::Px(p) = v {
            *p = (*p + d).max(least);
        }
    };

    let delta = drag.delta;

    match handle {
        WindowHandle::Title(w) => {
            if let Ok(mut node) = windows.get_mut(w) {
                nudge(&mut node.left, delta.x, f32::MIN);
                nudge(&mut node.top, delta.y, f32::MIN);
            }
        }
        WindowHandle::Resize(w) => {
            if let Ok(mut node) = windows.get_mut(w) {
                nudge(&mut node.width, delta.x, 64.0);
                nudge(&mut node.height, delta.y, 48.0);
            }
        }
    }
}
