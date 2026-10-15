use bevy::{input_focus::tab_navigation::TabIndex, picking::hover::Hovered, prelude::*, ui_widgets::{Slider, SliderPrecision, SliderRange, SliderStep, SliderThumb, SliderValue, TrackClick, slider_self_update}};

use crate::ui::{BAR, TEXT_SIZE, THUMB, TRACK, labelled_row, text_bundle};

const CHAR_WIDTH: f32 = 0.62;

#[derive(Component)]
pub(super) struct SliderLabel {
    slider: Entity,
    label: String,
}

pub fn slider_int(commands: &mut Commands, parent: Entity, label: &str, value: i32, (min, max): (i32, i32), width: f32) -> Entity {
    let row = labelled_row(commands, parent);

    let slider = commands
        .spawn((
            Node { width: px(width), height: px(24), ..default() },
            BackgroundColor(TRACK),
            Slider { track_click: TrackClick::Snap, ..default() },
            SliderValue(value as f32),
            SliderRange::new(min as f32, max as f32),
            SliderStep(1.0),
            SliderPrecision(0),
            TabIndex(0),
            Hovered::default(),
            ChildOf(row),
        ))
        .observe(slider_self_update)
        .id();

    commands.spawn((Node { position_type: PositionType::Absolute, width: px(BAR), height: percent(100), ..default() }, BackgroundColor(THUMB), SliderThumb, ChildOf(slider)));
    let longest = format!("{label}: {}", if min.to_string().len() > max.to_string().len() { min } else { max }).chars().count();
    commands.spawn((
        Node { min_width: px(longest as f32 * TEXT_SIZE * CHAR_WIDTH), ..default() },
        text_bundle("", TEXT_SIZE, Color::WHITE),
        SliderLabel { slider, label: label.to_string() },
        ChildOf(row),
    ));

    slider
}

pub(super) fn slider_thumbs(sliders: Query<(&SliderValue, &SliderRange, &Children), Or<(Changed<SliderValue>, Changed<SliderRange>)>>, mut thumbs: Query<&mut Node, With<SliderThumb>>) {
    for (value, range, children) in &sliders {
        let t = range.thumb_position(value.0).clamp(0.0, 1.0);

        for &child in children {
            if let Ok(mut node) = thumbs.get_mut(child) {
                node.left = percent(t * 100.0);
                node.margin.left = px(-BAR * t);
            }
        }
    }
}

pub(super) fn slider_labels(sliders: Query<&SliderValue>, mut labels: Query<(&SliderLabel, &mut Text)>) {
    for (l, mut text) in &mut labels {
        let Ok(value) = sliders.get(l.slider) else { continue };

        let want = format!("{}: {}", l.label, value.0.round() as i32);

        if text.0 != want {
            text.0 = want;
        }
    }
}
