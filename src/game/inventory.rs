use bevy::{platform::collections::HashMap, prelude::*};
use ferrosa_protocol::clientbound::game::ItemKind;

use super::{
    controls::{Control, InputState},
    hud::{self, Stick, DIALLING, HudScreen, align, anchored, in_hand, is_phone, own, rect, spawn_screen, text},
    items::ItemTypes,
    netgame::{INVENTORY_SLOTS, NetGame},
};
use crate::{fonts::GameFonts, menu::Screen};

const ICONS: [(ItemKind, &str); 26] = [
    (ItemKind::SoccerBall, "icon-soccer_ball"),
    (ItemKind::DiskWhite, "icon-disk-white"),
    (ItemKind::DiskBlack, "icon-disk-black"),
    (ItemKind::DiskGreen, "icon-disk-green"),
    (ItemKind::DiskBlue, "icon-disk-blue"),
    (ItemKind::DiskGold, "icon-disk-gold"),
    (ItemKind::DiskRed, "icon-disk-red"),
    (ItemKind::Computer, "icon-keyboard"),
    (ItemKind::Briefcase, "icon-case-empty"),
    (ItemKind::BriefcaseOpen, "icon-case-empty"),
    (ItemKind::CashRound, "icon-stack"),
    (ItemKind::CashWorld, "icon-stack"),
    (ItemKind::Bandage, "icon-bandage"),
    (ItemKind::Pistol, "icon-pistol"),
    (ItemKind::PistolMag, "icon-clip-pistol"),
    (ItemKind::Phone, "icon-phone"),
    (ItemKind::Ak47, "icon-ak47"),
    (ItemKind::Ak47Mag, "icon-clip-ak47"),
    (ItemKind::M16, "icon-m16"),
    (ItemKind::M16Mag, "icon-clip-m16"),
    (ItemKind::Mp5, "icon-mp5"),
    (ItemKind::Mp5Mag, "icon-clip-mp5"),
    (ItemKind::Uzi, "icon-uzi"),
    (ItemKind::UziMag, "icon-clip-uzi"),
    (ItemKind::Key, "icon-key"),
    (ItemKind::Grenade, "icon-grenade"),
];
const BOX: Vec2 = Vec2::new(128.0, 32.0);
const HANDS: usize = 2;
const POCKET_ITEMS: usize = 2;
const SWAPPING_HANDS: i32 = 1;
const POCKETING: i32 = 2;

#[derive(Resource)]
struct ItemIcons(HashMap<ItemKind, Handle<Image>>);

#[derive(Clone, PartialEq, Default)]
struct View {
    slots: Vec<Vec<(Option<ItemKind>, i32)>>,
    selected: Option<usize>,
}

#[derive(Component)]
struct InventoryPanel;

#[derive(Component)]
struct InventoryScreen;

#[derive(Component)]
struct Flying;

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_icons)
            .add_systems(OnEnter(Screen::InGame), spawn_inventory)
            .add_systems(Update, (show_inventory, fly_item).run_if(in_state(Screen::InGame)).run_if(resource_exists::<NetGame>));
    }
}

fn load_icons(mut commands: Commands, assets: Res<AssetServer>) {
    let icons = ICONS.iter().map(|&(kind, name)| (kind, assets.load(format!("icons/{name}.png")))).collect();

    commands.insert_resource(ItemIcons(icons));
}

fn slot_at(slot: usize) -> Vec2 {
    match slot {
        0 => Vec2::new(608.0, 492.0),
        1 => Vec2::new(416.0, 492.0),
        _ => Vec2::new((slot - 2) as f32 * 140.0 + 212.0, 544.0),
    }
}

fn entry(code: i32) -> (Option<ItemKind>, i32) {
    (ItemKind::try_from((code & 0xff) as u8).ok(), code >> 8)
}

fn spawn_inventory(mut commands: Commands) {
    let screen = spawn_screen(&mut commands, (HudScreen, InventoryScreen), Stick::Bottom);

    commands.spawn((rect(align::BOTTOM_CENTRE, Vec2::ZERO, BOX), ImageNode::default(), Flying, Visibility::Hidden, Pickable::IGNORE, ChildOf(screen)));
}

fn show_inventory(
    mut commands: Commands,
    net: Res<NetGame>,
    input: Res<InputState>,
    types: Res<ItemTypes>,
    icons: Res<ItemIcons>,
    fonts: Res<GameFonts>,
    screen: Single<Entity, With<InventoryScreen>>,
    panels: Query<Entity, With<InventoryPanel>>,
    mut last: Local<Option<View>>,
) {
    let view = own(&net).map(|(p, h, _)| {
        let dialling = in_hand(&net, h, 0).is_some_and(|(item, kind)| is_phone(kind) && net.items[item].phone.0 == DIALLING);
        let selected = (HANDS..INVENTORY_SLOTS).find(|&slot| !dialling && input.holding(Control::Row((slot - 1) as u8)));

        View { slots: p.display_inventory.iter().map(|s| s.iter().map(|&c| entry(c)).collect()).collect(), selected }
    });

    if view == *last {
        return;
    }

    panels.iter().for_each(|e| commands.entity(e).despawn());

    if let Some(view) = &view {
        let panel = commands.spawn((Node { position_type: PositionType::Absolute, ..default() }, InventoryPanel, Pickable::IGNORE, ChildOf(*screen))).id();
        spawn_slots(&mut commands, panel, view, &types, &icons, &fonts);
    }

    *last = view;
}

fn spawn_slots(commands: &mut Commands, panel: Entity, view: &View, types: &ItemTypes, icons: &ItemIcons, fonts: &GameFonts) {
    let shade = BackgroundColor(hud::colour([0.0, 0.0, 0.0, 0.25]));
    let frame = |at: Vec2, border: UiRect| (Node { border, ..rect(align::BOTTOM_CENTRE, at, BOX) }, shade, BorderColor::all(Color::WHITE));
    let small = |s: String, at: Vec2, size: f32| {
        let (node, shift) = anchored(align::BOTTOM, at);
        (node, shift, text(s, &fonts.osd_white, size, Color::WHITE))
    };

    let icon = |kind: Option<ItemKind>, at: Vec2, scale: f32, tint: Color| {
        let image = kind.and_then(|k| icons.0.get(&k)).cloned()?;
        Some((rect(align::BOTTOM_CENTRE, at, BOX * scale), ImageNode::new(image).with_color(tint), Pickable::IGNORE))
    };

    for (slot, items) in view.slots.iter().enumerate().take(HANDS) {
        let at = slot_at(slot);

        commands.spawn((frame(at + Vec2::X * 8.0, UiRect::all(px(1.0))), ChildOf(panel)));

        for &(kind, count) in items {
            let pin_pulled = count == 0 && kind == Some(ItemKind::Grenade);

            if let Some(bundle) = icon(kind, at + Vec2::X * 8.0, 1.0, if pin_pulled { Color::srgb(1.0, 0.0, 0.0) } else { Color::WHITE }) {
                commands.spawn((bundle, ChildOf(panel)));
            }

            let rounds = kind.and_then(|k| types.0.get(k as usize)).is_some_and(|t| t.magazine_ammo > 0);

            if rounds {
                commands.spawn((small(count.to_string(), at + Vec2::X * 16.0, 8.0), ChildOf(panel)));
            } else if count > 0 && kind != Some(ItemKind::BriefcaseOpen) {
                let note = if count == 1 { "+magazine" } else { "+empty magazine" };

                commands.spawn((small(note.to_string(), at + Vec2::X * 8.0, 8.0), ChildOf(panel)));
            }
        }
    }

    for (slot, items) in view.slots.iter().enumerate().skip(HANDS) {
        let at = slot_at(slot);

        for row in 0..POCKET_ITEMS {
            let border = if row == 0 { UiRect { bottom: px(0.0), ..UiRect::all(px(1.0)) } } else { UiRect { top: px(0.0), ..UiRect::all(px(1.0)) } };

            commands.spawn((frame(at + Vec2::new(8.0, 32.0 * row as f32), border), ChildOf(panel)));
        }

        commands.spawn((small((slot - 1).to_string(), at - Vec2::Y * 32.0, 12.0), ChildOf(panel)));

        for (row, &(kind, _)) in items.iter().enumerate() {
            let selected = row == 0 && view.selected == Some(slot);
            let (scale, tint) = if selected { (1.125, Color::srgb(0.0, 1.0, 0.0)) } else { (1.0, Color::WHITE) };

            if let Some(bundle) = icon(kind, at + Vec2::new(8.0, 32.0 * row as f32), scale, tint) {
                commands.spawn((bundle, ChildOf(panel)));
            }
        }
    }
}

fn fly_item(net: Res<NetGame>, icons: Res<ItemIcons>, flying: Single<(&mut Node, &mut ImageNode, &mut Visibility), With<Flying>>) {
    let (mut node, mut image, mut visibility) = flying.into_inner();

    let route = own(&net).and_then(|(p, _, own)| {
        let hand = own.action_hand as usize & 7;

        let (from, to) = match own.action_type {
            POCKETING if p.display_inventory.get(hand).is_some_and(|s| !s.is_empty()) => (hand, own.action_slot as usize + HANDS),
            POCKETING => (own.action_slot as usize + HANDS, hand),
            SWAPPING_HANDS => (hand, (own.action_hand ^ 1) as usize & 7),
            _ => return None,
        };

        let (kind, _) = entry(*p.display_inventory.get(from)?.first()?);
        let to = to.min(INVENTORY_SLOTS - 1);
        let t = own.action_duration as f32 / 100.0;

        Some((icons.0.get(&kind?)?.clone(), slot_at(from).lerp(slot_at(to), t)))
    });

    let Some((icon, at)) = route else {
        visibility.set_if_neq(Visibility::Hidden);
        return;
    };

    *node = rect(align::BOTTOM_CENTRE, at, BOX);
    visibility.set_if_neq(Visibility::Inherited);

    if image.image != icon {
        image.image = icon;
    }
}
