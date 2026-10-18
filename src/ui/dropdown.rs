use bevy::{picking::hover::Hovered, prelude::*, ui_widgets::{ReselectListRow, ValueChange, popover::{Popover, PopoverAlign, PopoverPlacement, PopoverSide}}};

use crate::ui::{BAR, ITEM_TEXT_SIZE, POPUP, POPUP_Z, DROPDOWN, TEXT_SIZE, labelled_row, listbox, text_bundle};

#[derive(Component)]
pub(super) struct DropdownPopup(Entity);

#[derive(Component)]
pub(super) struct Closing;

fn close(commands: &mut Commands, popup: Entity) {
    commands.entity(popup).insert((Closing, Visibility::Hidden));
}

pub(super) fn despawn_closed(popups: Query<Entity, With<Closing>>, mut commands: Commands) {
    for popup in &popups {
        commands.entity(popup).despawn();
    }
}

pub fn dropdown(commands: &mut Commands, parent: Entity, label: &str, items: &[String], chosen: usize, size: Vec2, list_size: Vec2) -> Entity {
    let row = labelled_row(commands, parent);
    let dropdown = commands
        .spawn((
            Node { width: px(size.x), height: px(size.y), justify_content: JustifyContent::Center, align_items: AlignItems::Center, overflow: Overflow::clip(), ..default() },
            BackgroundColor(DROPDOWN),
            Hovered::default(),
            Dropdown { items: items.to_vec(), chosen, list_size, popup: None },
            ChildOf(row),
        ))
        .observe(dropdown_wheel)
        .observe(dropdown_click)
        .id();

    let shown = items.get(chosen).map_or("", String::as_str);

    commands.spawn((text_bundle(shown, ITEM_TEXT_SIZE, Color::WHITE), TextLayout::no_wrap(), DropdownText(dropdown), Pickable::IGNORE, ChildOf(dropdown)));
    commands.spawn((text_bundle(label, TEXT_SIZE, Color::WHITE), ChildOf(row)));

    dropdown
}

fn dropdown_wheel(scroll: On<PointerScroll>, mut dropdowns: Query<&mut Dropdown>, mut texts: Query<(&DropdownText, &mut Text)>, mut commands: Commands) {
    let Ok(mut s) = dropdowns.get_mut(scroll.entity) else { return };
    let count = s.items.len();

    if count == 0 || scroll.y == 0.0 {
        return;
    }

    let chosen = if scroll.y < 0.0 { (s.chosen + count - 1) % count } else { (s.chosen + 1) % count };

    choose(scroll.entity, &mut s, chosen, &mut texts, &mut commands);
}

fn dropdown_click(click: On<PointerClick>, mut dropdowns: Query<(&mut Dropdown, &ChildOf)>, mut commands: Commands) {
    let dropdown = click.entity;
    let Ok((mut s, &ChildOf(row))) = dropdowns.get_mut(dropdown) else { return };

    if let Some(popup) = s.popup.take() {
        close(&mut commands, popup);
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
            DropdownPopup(dropdown),
            ChildOf(row),
        ))
        .id();

    let item = Vec2::new(s.list_size.x, 20.0);
    let list = listbox(&mut commands, popup, &s.items, Some(s.chosen), Vec2::new(s.list_size.x + BAR, s.list_size.y), item);

    commands.entity(list).observe(dropdown_pick).observe(dropdown_reselect);

    s.popup = Some(popup);
}

#[derive(Component)]
pub struct Dropdown {
    items: Vec<String>,
    chosen: usize,
    list_size: Vec2,
    popup: Option<Entity>,
}

fn dropdown_pick(
    picked: On<ValueChange<usize>>,
    parents: Query<&ChildOf>,
    popups: Query<&DropdownPopup>,
    mut dropdowns: Query<&mut Dropdown>,
    mut texts: Query<(&DropdownText, &mut Text)>,
    mut commands: Commands,
) {
    let Some((popup, dropdown)) = parents.iter_ancestors(picked.source).find_map(|a| popups.get(a).ok().map(|p| (a, p.0))) else { return };

    close(&mut commands, popup);

    if let Ok(mut s) = dropdowns.get_mut(dropdown) {
        s.popup = None;

        choose(dropdown, &mut s, picked.value, &mut texts, &mut commands);
    }
}

fn dropdown_reselect(picked: On<ReselectListRow>, parents: Query<&ChildOf>, popups: Query<&DropdownPopup>, mut dropdowns: Query<&mut Dropdown>, mut commands: Commands) {
    let Some((popup, dropdown)) = parents.iter_ancestors(picked.source).find_map(|a| popups.get(a).ok().map(|p| (a, p.0))) else { return };

    close(&mut commands, popup);

    if let Ok(mut s) = dropdowns.get_mut(dropdown) {
        s.popup = None;
    }
}

#[derive(Component)]
struct DropdownText(Entity);

fn choose(dropdown: Entity, s: &mut Dropdown, chosen: usize, texts: &mut Query<(&DropdownText, &mut Text)>, commands: &mut Commands) {
    s.chosen = chosen;

    for (_, mut text) in texts.iter_mut().filter(|(of, _)| of.0 == dropdown) {
        text.0 = s.items.get(chosen).cloned().unwrap_or_default();
    }

    commands.trigger(ValueChange { source: dropdown, value: chosen, is_final: true });
}

pub(super) fn close_dropdowns(mouse: Res<ButtonInput<MouseButton>>, popups: Query<(Entity, &DropdownPopup, &Hovered), Without<Closing>>, mut dropdowns: Query<(&mut Dropdown, &Hovered)>, mut commands: Commands) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    for (popup, of, hovered) in &popups {
        let Ok((mut s, dropdown_hovered)) = dropdowns.get_mut(of.0) else { continue };

        if !hovered.get() && !dropdown_hovered.get() {
            close(&mut commands, popup);
            s.popup = None;
        }
    }
}
