use bevy::{input_focus::tab_navigation::TabIndex, picking::hover::Hovered, prelude::*, ui::Checked, ui_widgets::{Checkbox, checkbox_self_update}};

use crate::ui::{CHECK_FRAME, TEXT_SIZE, text_bundle};

#[derive(Component)]
pub(super) struct TickOf(Entity);

pub fn checkbox(commands: &mut Commands, parent: Entity, label: &str, checked: bool) -> Entity {
    let row = commands
        .spawn((Node { align_items: AlignItems::Center, column_gap: px(8), ..default() }, Checkbox, TabIndex(0), Hovered::default(), ChildOf(parent)))
        .observe(checkbox_self_update)
        .id();

    if checked {
        commands.entity(row).insert(Checked);
    }

    let frame = commands
        .spawn((
            Node { width: px(20), height: px(20), border: UiRect::all(px(2)), padding: UiRect::all(px(3)), ..default() },
            BorderColor::all(CHECK_FRAME),
            BackgroundColor(Color::BLACK),
            ChildOf(row),
        ))
        .id();

    commands.spawn((Node { width: percent(100), height: percent(100), ..default() }, BackgroundColor(Color::BLACK), TickOf(row), ChildOf(frame)));
    commands.spawn((text_bundle(label, TEXT_SIZE, Color::WHITE), ChildOf(row)));

    row
}

pub(super) fn checkbox_ticks(boxes: Query<(&Hovered, Has<Checked>), With<Checkbox>>, mut ticks: Query<(&TickOf, &mut BackgroundColor)>) {
    for (of, mut colour) in &mut ticks {
        let Ok((hovered, checked)) = boxes.get(of.0) else { continue };

        let shade = match (checked, hovered.get()) {
            (true, true) => 1.0,
            (true, false) => 0.875,
            (false, true) => 0.25,
            (false, false) => 0.0,
        };

        let want = Color::srgb(shade, shade, shade);

        if colour.0 != want {
            colour.0 = want;
        }
    }
}