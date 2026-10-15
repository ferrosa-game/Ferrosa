use bevy::{picking::hover::Hovered, prelude::*, ui_widgets::{ReselectListRow, ValueChange, popover::{Popover, PopoverAlign, PopoverPlacement, PopoverSide}}};

use crate::ui::{BAR, ITEM_TEXT_SIZE, POPUP, POPUP_Z, SPINNER, TEXT_SIZE, labelled_row, listbox, text_bundle};

#[derive(Component)]
pub(super) struct SpinnerPopup(Entity);

pub fn spinner(commands: &mut Commands, parent: Entity, label: &str, items: &[String], chosen: usize, size: Vec2, list_size: Vec2) -> Entity {
    let row = labelled_row(commands, parent);
    let spinner = commands
        .spawn((
            Node { width: px(size.x), height: px(size.y), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
            BackgroundColor(SPINNER),
            Hovered::default(),
            Spinner { items: items.to_vec(), chosen, list_size, popup: None },
            ChildOf(row),
        ))
        .observe(spinner_wheel)
        .observe(spinner_click)
        .id();

    let shown = items.get(chosen).map_or("", String::as_str);

    commands.spawn((text_bundle(shown, ITEM_TEXT_SIZE, Color::WHITE), SpinnerText(spinner), Pickable::IGNORE, ChildOf(spinner)));
    commands.spawn((text_bundle(label, TEXT_SIZE, Color::WHITE), ChildOf(row)));

    spinner
}

fn spinner_wheel(scroll: On<PointerScroll>, mut spinners: Query<&mut Spinner>, mut texts: Query<(&SpinnerText, &mut Text)>, mut commands: Commands) {
    let Ok(mut s) = spinners.get_mut(scroll.entity) else { return };
    let count = s.items.len();

    if count == 0 || scroll.y == 0.0 {
        return;
    }

    let chosen = if scroll.y < 0.0 { (s.chosen + count - 1) % count } else { (s.chosen + 1) % count };

    choose(scroll.entity, &mut s, chosen, &mut texts, &mut commands);
}

fn spinner_click(click: On<PointerClick>, mut spinners: Query<(&mut Spinner, &ChildOf)>, mut commands: Commands) {
    let spinner = click.entity;
    let Ok((mut s, &ChildOf(row))) = spinners.get_mut(spinner) else { return };

    if let Some(popup) = s.popup.take() {
        commands.entity(popup).despawn();
        return;
    }

    let popup = commands
        .spawn((
            Node { position_type: PositionType::Absolute, ..default() },
            Popover {
                positions: vec![
                    PopoverPlacement { side: PopoverSide::Bottom, align: PopoverAlign::Start, gap: 2.0 },
                    PopoverPlacement { side: PopoverSide::Top, align: PopoverAlign::Start, gap: 2.0 },
                ],
                window_margin: 4.0,
            },
            BackgroundColor(POPUP),
            GlobalZIndex(POPUP_Z),
            Hovered::default(),
            SpinnerPopup(spinner),
            ChildOf(row),
        ))
        .id();

    let item = Vec2::new(s.list_size.x, 20.0);
    let list = listbox(&mut commands, popup, &s.items, Some(s.chosen), Vec2::new(s.list_size.x + BAR, s.list_size.y), item);

    commands.entity(list).observe(spinner_pick).observe(spinner_reselect);

    s.popup = Some(popup);
}

#[derive(Component)]
pub struct Spinner {
    items: Vec<String>,
    chosen: usize,
    list_size: Vec2,
    popup: Option<Entity>,
}

fn spinner_pick(
    picked: On<ValueChange<usize>>,
    parents: Query<&ChildOf>,
    popups: Query<&SpinnerPopup>,
    mut spinners: Query<&mut Spinner>,
    mut texts: Query<(&SpinnerText, &mut Text)>,
    mut commands: Commands,
) {
    let Some((popup, spinner)) = parents.iter_ancestors(picked.source).find_map(|a| popups.get(a).ok().map(|p| (a, p.0))) else { return };

    commands.entity(popup).despawn();

    if let Ok(mut s) = spinners.get_mut(spinner) {
        s.popup = None;

        choose(spinner, &mut s, picked.value, &mut texts, &mut commands);
    }
}

fn spinner_reselect(picked: On<ReselectListRow>, parents: Query<&ChildOf>, popups: Query<&SpinnerPopup>, mut spinners: Query<&mut Spinner>, mut commands: Commands) {
    let Some((popup, spinner)) = parents.iter_ancestors(picked.source).find_map(|a| popups.get(a).ok().map(|p| (a, p.0))) else { return };

    commands.entity(popup).despawn();

    if let Ok(mut s) = spinners.get_mut(spinner) {
        s.popup = None;
    }
}

#[derive(Component)]
struct SpinnerText(Entity);

fn choose(spinner: Entity, s: &mut Spinner, chosen: usize, texts: &mut Query<(&SpinnerText, &mut Text)>, commands: &mut Commands) {
    s.chosen = chosen;

    for (_, mut text) in texts.iter_mut().filter(|(of, _)| of.0 == spinner) {
        text.0 = s.items.get(chosen).cloned().unwrap_or_default();
    }

    commands.trigger(ValueChange { source: spinner, value: chosen, is_final: true });
}

pub(super) fn close_spinners(mouse: Res<ButtonInput<MouseButton>>, popups: Query<(Entity, &SpinnerPopup, &Hovered)>, mut spinners: Query<(&mut Spinner, &Hovered)>, mut commands: Commands) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    for (popup, of, hovered) in &popups {
        let Ok((mut s, spinner_hovered)) = spinners.get_mut(of.0) else { continue };

        if !hovered.get() && !spinner_hovered.get() {
            commands.entity(popup).despawn();
            s.popup = None;
        }
    }
}
