use bevy::{prelude::*, text::EditableText};

use crate::ui::{TEXT_SIZE, text_bundle, text_field};

#[derive(Component)]
pub(super) struct MaskOf(Entity);

pub(super) fn password_masks(inputs: Query<&EditableText>, mut masks: Query<(&MaskOf, &mut Text)>) {
    for (of, mut text) in &mut masks {
        let Ok(editable) = inputs.get(of.0) else { continue };

        let want = "*".repeat(editable.value().chars().count().min(0x1f));

        if text.0 != want {
            text.0 = want;
        }
    }
}

pub fn password_input(commands: &mut Commands, parent: Entity, label: &str, text: &str, max_len: usize, width: f32) -> Entity {
    let (field, input) = text_field(commands, parent, label, text, max_len, width);

    commands.entity(input).insert(TextColor(Color::NONE));
    commands.spawn((
        Node { position_type: PositionType::Absolute, left: px(4), ..default() },
        text_bundle("", TEXT_SIZE, Color::WHITE),
        Pickable::IGNORE,
        MaskOf(input),
        ChildOf(field),
    ));

    input
}