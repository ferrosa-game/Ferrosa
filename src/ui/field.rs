use bevy::{input::keyboard::KeyboardInput, input_focus::{FocusedInput, InputFocus, tab_navigation::TabIndex}, picking::hover::Hovered, prelude::*, text::{EditableText, EditableTextFilter, TextEditChange}, ui_widgets::{SelectAllOnFocus, TextInput, ValueChange}};

use crate::ui::{FIELD, LastNumber, TEXT_SIZE, labelled_row, scan_int, text_bundle};

#[derive(Component)]
pub struct Field;

#[derive(Component, Clone, Copy)]
enum NumberRange {
    Int(i32, i32),
    #[allow(unused)]
    Float(f32, f32),
}

pub fn text_input(commands: &mut Commands, parent: Entity, label: &str, text: &str, max_len: usize, width: f32) -> Entity {
    text_field(commands, parent, label, text, max_len, width).1
}

pub fn text_field(commands: &mut Commands, parent: Entity, label: &str, text: &str, max_len: usize, width: f32) -> (Entity, Entity) {
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

#[allow(unused)]
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

    let Ok((range, mut editable, mut last)) = inputs.get_mut(entered.source) else { 
        return;
    };

    let shown = match *range {
        NumberRange::Int(min, max) => {
            let v = scan_int(&entered.value)
                .unwrap_or(last.0 as i32)
                .clamp(min, max);
            
            last.0 = v as f64;
            commands.trigger(ValueChange { source: entered.source, value: v, is_final: true });
            
            v.to_string()
        }
        NumberRange::Float(min, max) => {
            let v = entered.value
                .trim()
                .parse::<f32>()
                .unwrap_or(last.0 as f32)
                .clamp(min, max);

            last.0 = v as f64;
            commands.trigger(ValueChange { source: entered.source, value: v, is_final: true });

            format!("{v:.3}")
        }
    };

    editable.editor_mut().set_text(&shown);
}