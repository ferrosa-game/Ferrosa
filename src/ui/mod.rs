//! The Sub Rosa GUI's widgets (ferrosa-gui) as Bevy UI: labels, buttons, checkboxes, radio buttons, integer sliders,
//! text, password, number and float inputs, list boxes, spinners and windows. Each is spawned under a parent and
//! reports through Bevy's widget events on the entity returned: [`Activate`] for buttons, [`ValueChange`] for the
//! rest (`bool` checkboxes, the chosen button's `Entity` on a radio group, `f32` sliders, `String` text inputs,
//! `i32`/`f32` number inputs once entered, `usize` list boxes and spinners).

use bevy::{
    input::keyboard::KeyboardInput,
    input_focus::{FocusedInput, InputFocus, tab_navigation::TabIndex},
    picking::
        hover::Hovered
    ,
    prelude::*,
    text::{EditableText, EditableTextFilter, TextEditChange},
    ui::{Checked, Pressed, Selected},
    ui_widgets::{
        Activate, Button, ControlOrientation, ListBox, ListItem, RadioButton,
        ScrollArea, Scrollbar, ScrollbarThumb, SelectAllOnFocus, Slider, TextInput, ValueChange, listbox_update_selection,
    },
};

use crate::ui::{checkbox::checkbox_ticks, slider::{slider_labels, slider_thumbs}, spinner::{Spinner, close_spinners}, text::{MaskOf, password_masks}, window::WindowOrder};

pub mod radio;
pub mod slider;
pub mod text;
pub mod checkbox;
pub mod spinner;
pub mod window;

pub const TEXT_SIZE: f32 = 16.0;
const ITEM_TEXT_SIZE: f32 = 13.0;
pub const BAR: f32 = 16.0;

const BUTTON: Color = Color::srgb(0.12, 0.16, 0.2);
const BUTTON_HOVERED: Color = Color::srgb(0.22, 0.28, 0.34);
const BUTTON_PRESSED: Color = Color::srgb(0.3, 0.38, 0.46);
const BUTTON_CHECKED: Color = Color::srgb(0.32, 0.42, 0.52);
const FIELD: Color = Color::srgba(0.1875, 0.1875, 0.1875, 0.5);
const FIELD_HOVERED: Color = Color::srgba(0.3125, 0.3125, 0.3125, 0.5);
const TRACK: Color = Color::srgb(0.25, 0.25, 0.25);
const TRACK_HOVERED: Color = Color::srgb(0.375, 0.375, 0.375);
const THUMB: Color = Color::srgb(0.5, 0.5, 0.5);
const CHECK_FRAME: Color = Color::srgb(0.875, 0.875, 0.875);
const LIST: Color = Color::srgba(0.125, 0.125, 0.125, 0.85);
const ROW_CHOSEN: Color = Color::srgba(0.75, 0.75, 0.75, 0.5);
const ROW_HOVERED: Color = Color::srgba(0.625, 0.625, 0.625, 0.5);
const SPINNER: Color = Color::srgba(0.5, 0.5, 0.5, 0.5);
const SPINNER_HOVERED: Color = Color::srgba(0.75, 0.75, 0.75, 0.5);
const POPUP: Color = Color::srgb(0.08, 0.08, 0.08);
const POPUP_Z: i32 = 1 << 20;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WindowOrder>()
            .add_systems(Update, (shortcuts, button_colours, checkbox_ticks, field_colours, slider_thumbs, slider_labels, row_colours, password_masks, close_spinners));
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

#[derive(Component)]
struct Field;

#[derive(Component, Clone, Copy)]
enum NumberRange {
    Int(i32, i32),
    Float(f32, f32),
}

pub fn text_input(commands: &mut Commands, parent: Entity, label: &str, text: &str, max_len: usize, width: f32) -> Entity {
    text_field(commands, parent, label, text, max_len, width).1
}

fn text_field(commands: &mut Commands, parent: Entity, label: &str, text: &str, max_len: usize, width: f32) -> (Entity, Entity) {
    let row = labelled_row(commands, parent);

    let field = commands
        .spawn((
            Node { width: px(width), height: px(24), padding: UiRect::horizontal(px(4)), align_items: AlignItems::Center, overflow: Overflow::clip(), ..default() },
            BackgroundColor(FIELD),
            Field,
            Hovered::default(),
            ChildOf(row),
        ))
        .id();

    let mut editable = EditableText::new(text);

    editable.max_characters = Some(max_len);

    let input = commands
        .spawn((Node { width: percent(100), ..default() }, TextInput, editable, TextFont::from_font_size(TEXT_SIZE), SelectAllOnFocus, TabIndex(0), ChildOf(field)))
        .observe(end_edit_on_return)
        .observe(report_edit)
        .id();

    commands.spawn((text_bundle(label, TEXT_SIZE, Color::WHITE), ChildOf(row)));

    (field, input)
}

pub fn number_input(commands: &mut Commands, parent: Entity, label: &str, value: i32, (min, max): (i32, i32), width: f32) -> Entity {
    let input = text_input(commands, parent, label, &value.to_string(), 0x18, width);

    commands.entity(input).insert((NumberRange::Int(min, max), LastNumber(value as f64), EditableTextFilter::new(|c| c.is_ascii_digit() || c == '-' || c == '+'))).observe(enter_number);

    input
}

pub fn float_input(commands: &mut Commands, parent: Entity, label: &str, value: f32, (min, max): (f32, f32), width: f32) -> Entity {
    let input = text_input(commands, parent, label, &format!("{value:.3}"), 0x18, width);

    commands
        .entity(input)
        .insert((NumberRange::Float(min, max), LastNumber(value as f64), EditableTextFilter::new(|c| c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E'))))
        .observe(enter_number);

    input
}

fn end_edit_on_return(mut key: On<FocusedInput<KeyboardInput>>, texts: Query<&EditableText>, mut focus: ResMut<InputFocus>, mut commands: Commands) {
    let input = &key.event().input;

    if input.state.is_pressed() && matches!(input.key_code, KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Escape) {
        let entity = key.focused_entity;

        key.propagate(false);
        focus.clear();

        if let Ok(text) = texts.get(entity) {
            commands.trigger(ValueChange { source: entity, value: text.value().to_string(), is_final: true });
        }
    }
}

fn report_edit(edit: On<TextEditChange>, focus: Res<InputFocus>, texts: Query<&EditableText>, mut commands: Commands) {
    let entity = edit.event_target();

    if focus.get() != Some(entity) {
        return;
    }

    if let Ok(text) = texts.get(entity) {
        commands.trigger(ValueChange { source: entity, value: text.value().to_string(), is_final: false });
    }
}

fn enter_number(entered: On<ValueChange<String>>, mut inputs: Query<(&NumberRange, &mut EditableText, &mut LastNumber)>, mut commands: Commands) {
    if !entered.is_final {
        return;
    }

    let Ok((range, mut editable, mut last)) = inputs.get_mut(entered.source) else { return };

    let (shown, report) = match *range {
        NumberRange::Int(min, max) => {
            let v = scan_int(&entered.value).unwrap_or(last.0 as i32).clamp(min, max);
            last.0 = v as f64;
            (v.to_string(), Ok(v))
        }
        NumberRange::Float(min, max) => {
            let mut v = entered.value.trim().parse::<f32>().unwrap_or(last.0 as f32);

            if !(min <= v) {
                v = min;
            }

            if !(v <= max) {
                v = max;
            }

            last.0 = v as f64;

            (format!("{v:.3}"), Err(v))
        }
    };

    editable.editor_mut().set_text(&shown);

    match report {
        Ok(v) => commands.trigger(ValueChange { source: entered.source, value: v, is_final: true }),
        Err(v) => commands.trigger(ValueChange { source: entered.source, value: v, is_final: true }),
    }
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
        commands.spawn((text_bundle(&items[i], ITEM_TEXT_SIZE, Color::WHITE), ChildOf(row)));
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
                Node { width: px(item.x), height: px(item.y), padding: UiRect::left(px(4)), align_items: AlignItems::Center, ..default() },
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

fn shortcuts(keys: Res<ButtonInput<KeyCode>>, focus: Res<InputFocus>, inputs: Query<(), With<TextInput>>, buttons: Query<(Entity, &Shortcut)>, mut commands: Commands) {
    if focus.get().is_some_and(|f| inputs.contains(f)) {
        return;
    }
    for (entity, Shortcut(key)) in &buttons {
        if keys.just_pressed(*key) {
            commands.trigger(Activate { entity });
        }
    }
}

fn button_colours(mut buttons: Query<(&Hovered, Has<Pressed>, Has<Checked>, &mut BackgroundColor), Or<(With<Button>, With<RadioButton>)>>) {
    for (hovered, pressed, checked, mut colour) in &mut buttons {
        let want = if pressed {
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

fn field_colours(mut fields: Query<(&Hovered, &mut BackgroundColor, Has<Field>, Has<Spinner>, Has<Slider>), Or<(With<Field>, With<Spinner>, With<Slider>)>>) {
    for (hovered, mut colour, field, spinner, _) in &mut fields {
        let want = match (field, spinner, hovered.get()) {
            (true, _, true) => FIELD_HOVERED,
            (true, _, false) => FIELD,
            (_, true, true) => SPINNER_HOVERED,
            (_, true, false) => SPINNER,
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