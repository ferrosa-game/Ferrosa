use bevy::{platform::collections::HashMap, prelude::*};
use ferrosa_protocol::clientbound::game::ItemKind;

use super::{
    controls::{Control, InputState},
    hud::{self, Stick, HudScreen, align, anchored, in_hand, is_phone, own, rect, set_text, shown, spawn_screen, text},
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
const CELLS: usize = 2;
const SWAPPING_HANDS: i32 = 1;
const POCKETING: i32 = 2;

#[derive(Resource)]
struct ItemIcons{
    icons: HashMap<ItemKind, Handle<Image>>,
    not_found: Handle<Image>
}

impl ItemIcons {
    fn of(&self, kind: Option<ItemKind>) -> Handle<Image> {
        kind.and_then(|k| self.icons.get(&k)).unwrap_or(&self.not_found).clone()
    }
}

#[derive(Component)]
struct InventoryScreen;

#[derive(Component)]
struct SlotFrame(usize);

#[derive(Component)]
struct SlotLabel(usize);

#[derive(Component)]
struct Cell(usize, usize);

#[derive(Component)]
struct CellNote(usize, usize);

#[derive(Component)]
struct AmmoBar(usize, usize);

#[derive(Component)]
struct GunClip(usize, usize);

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

    commands.insert_resource(ItemIcons {
        icons,
        not_found: assets.load("icons/icon-not-found.png")
    });
}

fn slot_at(slot: usize) -> Vec2 {
    match slot {
        0 => Vec2::new(616.0, 492.0),
        1 => Vec2::new(424.0, 492.0),
        _ => Vec2::new((slot - 2) as f32 * 140.0 + 220.0, 544.0),
    }
}

fn cell_at(slot: usize, row: usize) -> Vec2 {
    slot_at(slot) + if slot < 2 { Vec2::ZERO } else { Vec2::Y * 32.0 * row as f32 }
}

fn entry(code: i32) -> (Option<ItemKind>, i32) {
    (ItemKind::try_from((code & 0xff) as u8).ok(), code >> 8)
}

fn clip_of(types: &ItemTypes, kind: Option<ItemKind>) -> Option<ItemKind> {
    let gun = kind? as usize;
    let mag = types.0.iter().position(|t| t.magazine_ammo > 0 && t.can_mount_to.get(gun).is_some_and(|&m| m != 0))?;

    ItemKind::try_from(u8::try_from(mag).ok()?).ok()
}

fn item_name(types: &ItemTypes, kind: Option<ItemKind>) -> Option<String> {
    let name = &types.0.get(kind? as usize)?.name;

    (!name.is_empty()).then(|| name.clone())
}

fn spawn_inventory(mut commands: Commands, fonts: Res<GameFonts>) {
    let screen = spawn_screen(&mut commands, (HudScreen, InventoryScreen), Stick::Bottom);
    let frame = |at: Vec2, border: UiRect| {
        (Node { border, ..rect(align::BOTTOM_CENTRE, at, BOX) }, BackgroundColor(hud::colour([0.0, 0.0, 0.0, 0.25])), BorderColor::all(Color::WHITE), Pickable::IGNORE)
    };

    for slot in 0..INVENTORY_SLOTS {
        let at = slot_at(slot);

        if slot < 2 {
            commands.spawn((frame(at, UiRect::all(px(1.0))), SlotFrame(slot), ChildOf(screen)));
        } else {
            for row in 0..CELLS {
                let border = if row == 0 { UiRect { bottom: px(0.0), ..UiRect::all(px(1.0)) } } else { UiRect { top: px(0.0), ..UiRect::all(px(1.0)) } };

                commands.spawn((frame(cell_at(slot, row), border), SlotFrame(slot), ChildOf(screen)));
            }
        }

        let (node, shift) = anchored(align::BOTTOM_CENTRE, Vec2::new(at.x, at.y - if slot < 2 { BOX.y + 2.0 } else { 32.0 }));
        commands.spawn((node, shift, SlotLabel(slot), text("", &fonts.osd_white, 12.0, Color::WHITE), ChildOf(screen)));

        for row in 0..CELLS {
            let cell = cell_at(slot, row);

            commands.spawn((rect(align::BOTTOM_CENTRE, cell, BOX), UiTransform::default(), ImageNode::default(), Cell(slot, row), Visibility::Hidden, Pickable::IGNORE, ChildOf(screen)));

            if slot < 2 {
                let bar = rect(align::BOTTOM, cell + Vec2::new(1.0 - BOX.x * 0.5, -1.0), Vec2::new(0.0, 2.0));

                commands.spawn((bar, AmmoBar(slot, row), BackgroundColor(Color::NONE), Visibility::Hidden, Pickable::IGNORE, ChildOf(screen)));

                let clip = rect(align::BOTTOM, cell + Vec2::new(BOX.x * 0.5 - 49.0, -1.0), Vec2::new(48.0, 12.0));

                commands.spawn((clip, GunClip(slot, row), ImageNode::default(), Visibility::Hidden, Pickable::IGNORE, ChildOf(screen)));

                let (node, shift) = anchored(align::BOTTOM, cell + Vec2::new(8.0, -4.0));

                commands.spawn((node, shift, CellNote(slot, row), text("", &fonts.osd_white, 8.0, Color::WHITE), ChildOf(screen)));
            }
        }
    }

    commands.spawn((rect(align::BOTTOM_CENTRE, Vec2::ZERO, BOX), ImageNode::default(), Flying, Visibility::Hidden, Pickable::IGNORE, ChildOf(screen)));
}

fn show_inventory(
    net: Res<NetGame>,
    input: Res<InputState>,
    types: Res<ItemTypes>,
    icons: Res<ItemIcons>,
    mut parts: ParamSet<(
        Query<(&Cell, &mut ImageNode, &mut UiTransform, &mut Visibility)>,
        Query<(&CellNote, &mut Text, &mut Visibility)>,
        Query<(&SlotLabel, &mut Text, &mut TextColor, &mut Visibility)>,
        Query<(&SlotFrame, &mut BorderColor)>,
        Query<(&AmmoBar, &mut Node, &mut BackgroundColor, &mut Visibility)>,
        Query<(&GunClip, &mut ImageNode, &mut Visibility)>,
    )>,
) {
    let Some((p, h, _)) = own(&net) else { return };

    let dialling = in_hand(&net, h, 0).is_some_and(|(item, kind)| is_phone(kind) && net.items[item].phone.0 == 1);
    let selected = (2..INVENTORY_SLOTS).find(|&slot| !dialling && input.holding(Control::Row((slot - 1) as u8)));
    let holding = p.display_inventory[..2].iter().find(|s| !s.is_empty()).and_then(|s| s.first().map(|&c| entry(c).0));

    let fits = |slot: usize| match holding {
        Some(kind) => {
            let fits = kind.and_then(|k| types.0.get(k as usize)).is_some_and(|t| t.pockets.get(slot - 2).is_some_and(|&p| p != 0));

            fits && p.display_inventory[slot].len() < CELLS
        }
        None => true,
    };

    for (&Cell(slot, row), mut image, mut transform, mut v) in &mut parts.p0() {
        let Some((kind, count)) = p.display_inventory[slot].get(row).map(|&c| entry(c)) else {
            shown(&mut v, false);
            continue;
        };

        let pin_pulled = count == 0 && kind == Some(ItemKind::Grenade);
        let picked = row == 0 && selected == Some(slot);
        let icon = icons.of(kind);
        let tint = if pin_pulled {
            Color::srgb(1.0, 0.0, 0.0)
        } else if picked {
            Color::srgb(0.0, 1.0, 0.0)
        } else {
            Color::WHITE
        };
        let scale = Vec2::splat(if picked { 1.125 } else { 1.0 });

        shown(&mut v, true);

        if image.image != icon {
            image.image = icon;
        }

        if image.color != tint {
            image.color = tint;
        }

        if transform.scale != scale {
            transform.scale = scale;
        }
    }

    for (&CellNote(slot, row), mut t, mut v) in &mut parts.p1() {
        let rounds = p.display_inventory[slot].get(row).map(|&c| entry(c)).and_then(|(kind, count)| {
            let mag = kind.and_then(|k| types.0.get(k as usize)).is_some_and(|t| t.magazine_ammo > 0);

            mag.then(|| count.to_string())
        });

        set_text(&mut t, &mut v, rounds);
    }

    for (&SlotLabel(slot), mut t, mut colour, mut v) in &mut parts.p2() {
        let name = item_name(&types, p.display_inventory[slot].first().map(|&c| entry(c)).and_then(|(kind, _)| kind));
        let label = match slot {
            0 | 1 => name,
            _ => name.filter(|_| selected == Some(slot)).or(Some((slot - 1).to_string())),
        };
        let faded = hud::colour([1.0, 1.0, 1.0, if slot >= 2 && !fits(slot) { 0.25 } else { 1.0 }]);

        if colour.0 != faded {
            colour.0 = faded;
        }

        set_text(&mut t, &mut v, label);
    }

    for (&SlotFrame(slot), mut edge) in &mut parts.p3() {
        let faded = hud::colour([1.0, 1.0, 1.0, if slot >= 2 && !fits(slot) { 0.25 } else { 1.0 }]);

        if edge.top != faded {
            *edge = BorderColor::all(faded);
        }
    }

    for (&AmmoBar(slot, row), mut node, mut fill, mut v) in &mut parts.p4() {
        let loaded = p.display_inventory[slot].get(row).map(|&c| entry(c)).and_then(|(kind, count)| {
            let cap = kind.and_then(|k| types.0.get(k as usize)).map_or(0, |t| t.magazine_ammo);

            (cap > 0).then(|| count.clamp(0, cap) as f32 / cap as f32)
        });

        shown(&mut v, loaded.is_some());

        let Some(t) = loaded else { continue };
        let width = px((BOX.x - 2.0) * t);
        let colour = hud::colour([(2.0 - 2.0 * t).min(1.0), (2.0 * t).min(1.0), 0.0, 0.875]);

        if node.width != width {
            node.width = width;
        }

        if fill.0 != colour {
            fill.0 = colour;
        }
    }

    for (&GunClip(slot, row), mut image, mut v) in &mut parts.p5() {
        let clip = p.display_inventory[slot].get(row).map(|&c| entry(c)).and_then(|(kind, count)| {
            let gun = kind.and_then(|k| types.0.get(k as usize)).is_some_and(|t| t.is_gun);

            if !gun || count == 0 {
                return None;
            }

            Some((icons.of(Some(clip_of(&types, kind)?)), count == 1))
        });

        shown(&mut v, clip.is_some());

        let Some((icon, full)) = clip else { continue };
        let tint = if full { Color::WHITE } else { Color::srgb(1.0, 0.25, 0.25) };

        if image.image != icon {
            image.image = icon;
        }

        if image.color != tint {
            image.color = tint;
        }
    }
}

fn fly_item(net: Res<NetGame>, icons: Res<ItemIcons>, flying: Single<(&mut Node, &mut ImageNode, &mut Visibility), With<Flying>>) {
    let (mut node, mut image, mut visibility) = flying.into_inner();

    let route = own(&net).and_then(|(p, _, own)| {
        let hand = own.action_hand as usize & 7;

        let (from, to) = match own.action_type {
            POCKETING if p.display_inventory.get(hand).is_some_and(|s| !s.is_empty()) => (hand, own.action_slot as usize + 2),
            POCKETING => (own.action_slot as usize + 2, hand),
            SWAPPING_HANDS => (hand, (own.action_hand ^ 1) as usize & 7),
            _ => return None,
        };

        let (kind, _) = entry(*p.display_inventory.get(from)?.first()?);
        let to = to.min(INVENTORY_SLOTS - 1);
        let t = own.action_duration as f32 / 100.0;

        Some((icons.of(kind), slot_at(from).lerp(slot_at(to), t)))
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
