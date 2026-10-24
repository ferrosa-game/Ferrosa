use bevy::{
    input_focus::{InputFocus, tab_navigation::TabIndex},
    picking::
        hover::Hovered
    ,
    prelude::*,
    ui::{Checked, InteractionDisabled, Pressed, Selected},
    ui_widgets::{
        Activate, Button, ControlOrientation, ListBox, ListItem, RadioButton,
        ScrollArea, Scrollbar, ScrollbarThumb, Slider, TextInput, ValueChange, listbox_update_selection,
    },
};

use crate::ui::{checkbox::checkbox_ticks, dropdown::{Dropdown, close_dropdowns, despawn_closed}, field::Field, slider::{slider_labels, slider_thumbs}, text::password_masks, window::WindowOrder};

pub mod radio;
pub mod slider;
pub mod text;
pub mod checkbox;
pub mod dropdown;
pub mod window;
pub mod field;

pub const TEXT_SIZE: f32 = 16.0;
const ITEM_TEXT_SIZE: f32 = 13.0;
pub const BAR: f32 = 16.0;

const BUTTON: Color = Color::srgb(0.12, 0.16, 0.2);
const BUTTON_HOVERED: Color = Color::srgb(0.22, 0.28, 0.34);
const BUTTON_PRESSED: Color = Color::srgb(0.3, 0.38, 0.46);
const BUTTON_CHECKED: Color = Color::srgb(0.32, 0.42, 0.52);
const BUTTON_DISABLED: Color = Color::srgb(0.08, 0.09, 0.1);
const FIELD: Color = Color::srgba(0.1875, 0.1875, 0.1875, 0.5);
const FIELD_HOVERED: Color = Color::srgba(0.3125, 0.3125, 0.3125, 0.5);
const TRACK: Color = Color::srgb(0.25, 0.25, 0.25);
const TRACK_HOVERED: Color = Color::srgb(0.375, 0.375, 0.375);
const THUMB: Color = Color::srgb(0.5, 0.5, 0.5);
const LEVEL: Color = Color::srgb(0.3, 0.9, 0.3);
const CHECK_FRAME: Color = Color::srgb(0.875, 0.875, 0.875);
const LIST: Color = Color::srgba(0.125, 0.125, 0.125, 0.85);
const ROW_CHOSEN: Color = Color::srgba(0.75, 0.75, 0.75, 0.5);
const ROW_HOVERED: Color = Color::srgba(0.625, 0.625, 0.625, 0.5);
const DROPDOWN: Color = Color::srgba(0.5, 0.5, 0.5, 0.5);
const DROPDOWN_HOVERED: Color = Color::srgba(0.75, 0.75, 0.75, 0.5);
const POPUP: Color = Color::srgb(0.08, 0.08, 0.08);
const POPUP_Z: i32 = 1 << 20;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WindowOrder>()
            .add_systems(Update, (shortcuts, button_colours, checkbox_ticks, field_colours, slider_thumbs, slider_labels, row_colours, password_masks, close_dropdowns))
            .add_systems(Last, despawn_closed);
    }
}

#[derive(Component, Clone, Copy)]
pub struct Shortcut(pub KeyCode);

fn text_bundle(s: &str, size: f32, colour: Color) -> impl Bundle {
    (Text::new(s), TextFont::from_font_size(size), TextColor(colour))
}

pub fn label(commands: &mut Commands, parent: Entity, s: &str, size: f32, colour: Color) -> Entity {
    commands.spawn((text_bundle(s, size, colour), ChildOf(parent))).id()
}

fn labelled_row(commands: &mut Commands, parent: Entity) -> Entity {
    commands.spawn((Node { align_items: AlignItems::Center, column_gap: px(6), ..default() }, ChildOf(parent))).id()
}

fn button_node(size: Vec2) -> Node {
    Node {
        width: px(size.x),
        height: px(size.y),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        border_radius: BorderRadius::all(px(4)),
        ..default()
    }
}

fn button_text_size(height: f32) -> f32 {
    (height * 0.55).clamp(12.0, 22.0)
}

pub fn button(commands: &mut Commands, parent: Entity, label: &str, size: Vec2) -> Entity {
    let b = commands.spawn((button_node(size), BackgroundColor(BUTTON), Button, TabIndex(0), Hovered::default(), ChildOf(parent))).id();
    commands.spawn((text_bundle(label, button_text_size(size.y), Color::WHITE), ChildOf(b)));
    b
}

#[derive(Component, Default)]
struct LastNumber(f64);

fn scan_int(text: &str) -> Option<i32> {
    let t = text.trim_start();
    let end = t.char_indices().take_while(|&(k, c)| c.is_ascii_digit() || (k == 0 && (c == '-' || c == '+'))).map(|(k, c)| k + c.len_utf8()).last()?;
    t[..end].parse().ok()
}

#[derive(Component, Clone, Copy)]
pub struct ListRow(pub usize);

pub fn listbox(commands: &mut Commands, parent: Entity, items: &[String], chosen: Option<usize>, size: Vec2, item: Vec2) -> Entity {
    listbox_with(commands, parent, items.len(), chosen, size, item, |commands, row, i| {
        commands.spawn((text_bundle(&items[i], ITEM_TEXT_SIZE, Color::WHITE), TextLayout::no_wrap(), ChildOf(row)));
    })
}

pub fn listbox_with(commands: &mut Commands, parent: Entity, count: usize, chosen: Option<usize>, size: Vec2, item: Vec2, mut fill: impl FnMut(&mut Commands, Entity, usize)) -> Entity {
    let frame = commands.spawn((Node { width: px(size.x), height: px(size.y), ..default() }, ChildOf(parent))).id();
    let list = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                height: percent(100),
                flex_wrap: FlexWrap::Wrap,
                align_content: AlignContent::FlexStart,
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(LIST),
            ListBox,
            ScrollArea,
            TabIndex(0),
            ChildOf(frame),
        ))
        .observe(listbox_update_selection)
        .observe(report_row)
        .id();
    for i in 0..count {
        let row = commands
            .spawn((
                Node { width: px(item.x), height: px(item.y), padding: UiRect::left(px(4)), align_items: AlignItems::Center, overflow: Overflow::clip(), ..default() },
                BackgroundColor(Color::NONE),
                ListItem,
                ListRow(i),
                Hovered::default(),
                ChildOf(list),
            ))
            .id();
        if chosen == Some(i) {
            commands.entity(row).insert(Selected);
        }
        fill(commands, row, i);
    }
    scrollbar(commands, frame, list);
    list
}

pub fn scrollbar(commands: &mut Commands, parent: Entity, target: Entity) -> Entity {
    let bar = commands
        .spawn((Node { width: px(BAR), height: percent(100), flex_shrink: 0.0, ..default() }, BackgroundColor(TRACK), Scrollbar::new(target, ControlOrientation::Vertical, BAR), ChildOf(parent)))
        .id();
    commands.spawn((ScrollbarThumb::default(), BackgroundColor(THUMB), ChildOf(bar)));
    bar
}

fn report_row(picked: On<ValueChange<Entity>>, rows: Query<&ListRow>, mut commands: Commands) {
    if let Ok(&ListRow(i)) = rows.get(picked.value) {
        commands.trigger(ValueChange { source: picked.source, value: i, is_final: true });
    }
}

fn shortcuts(keys: Res<ButtonInput<KeyCode>>, focus: Res<InputFocus>, inputs: Query<(), With<TextInput>>, buttons: Query<(Entity, &Shortcut), Without<InteractionDisabled>>, mut commands: Commands) {
    if focus.get().is_some_and(|f| inputs.contains(f)) {
        return;
    }
    for (entity, Shortcut(key)) in &buttons {
        if keys.just_pressed(*key) {
            commands.trigger(Activate { entity });
        }
    }
}

fn button_colours(mut buttons: Query<(&Hovered, Has<Pressed>, Has<Checked>, Has<InteractionDisabled>, &mut BackgroundColor), Or<(With<Button>, With<RadioButton>)>>) {
    for (hovered, pressed, checked, disabled, mut colour) in &mut buttons {
        let want = if disabled {
            BUTTON_DISABLED
        } else if pressed {
            BUTTON_PRESSED
        } else if checked {
            BUTTON_CHECKED
        } else if hovered.get() {
            BUTTON_HOVERED
        } else {
            BUTTON
        };

        if colour.0 != want {
            colour.0 = want;
        }
    }
}

fn field_colours(mut fields: Query<(&Hovered, &mut BackgroundColor, Has<Field>, Has<Dropdown>, Has<Slider>), Or<(With<Field>, With<Dropdown>, With<Slider>)>>) {
    for (hovered, mut colour, field, dropdown, _) in &mut fields {
        let want = match (field, dropdown, hovered.get()) {
            (true, _, true) => FIELD_HOVERED,
            (true, _, false) => FIELD,
            (_, true, true) => DROPDOWN_HOVERED,
            (_, true, false) => DROPDOWN,
            (_, _, true) => TRACK_HOVERED,
            (_, _, false) => TRACK,
        };

        if colour.0 != want {
            colour.0 = want;
        }
    }
}

fn row_colours(mut rows: Query<(&Hovered, Has<Selected>, &mut BackgroundColor), With<ListItem>>) {
    for (hovered, selected, mut colour) in &mut rows {
        let want = if hovered.get() {
            ROW_HOVERED
        } else if selected {
            ROW_CHOSEN
        } else {
            Color::NONE
        };

        if colour.0 != want {
            colour.0 = want;
        }
    }
}