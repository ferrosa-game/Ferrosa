use crate::ui::{BUTTON, button_node, button_text_size, text_bundle};
use bevy::{input_focus::tab_navigation::TabIndex, picking::hover::Hovered, prelude::*, ui::Checked, ui_widgets::{RadioButton, RadioGroup, radio_self_update}};

pub fn radio_group(commands: &mut Commands, parent: Entity) -> Entity {
    commands.spawn((Node { column_gap: px(4), ..default() }, RadioGroup, TabIndex(0), ChildOf(parent))).observe(radio_self_update).id()
}

pub fn radio_button(commands: &mut Commands, group: Entity, label: &str, size: Vec2, checked: bool) -> Entity {
    let b = commands.spawn((button_node(size), BackgroundColor(BUTTON), RadioButton, Hovered::default(), ChildOf(group))).id();

    if checked {
        commands.entity(b).insert(Checked);
    }

    commands.spawn((text_bundle(label, button_text_size(size.y), Color::WHITE), ChildOf(b)));

    b
}