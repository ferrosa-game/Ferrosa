use bevy::prelude::*;
use ferrosa_protocol::{GameMode, Team, clientbound::game::MenuType};

use super::{align, anchored, own, set_text, shown, text};
use crate::{
    fonts::GameFonts,
    game::{escape_menu::GameMenu, netgame::NetGame},
    names::corporation_index,
};

const CORPORATION_MENUS: std::ops::RangeInclusive<i32> = 0xe..=0x1a;

#[derive(Component)]
pub(super) struct MenuLine;

#[derive(Component)]
pub(super) struct TabHint;

#[derive(Component)]
pub(super) struct SharesLine;

#[derive(Component)]
pub(super) struct BudgetLine;

pub(super) fn spawn(commands: &mut Commands, top_left: Entity, top: Entity, fonts: &GameFonts) {
    let white = Color::WHITE;

    let (node, shift) = anchored(align::TOP_LEFT, Vec2::new(4.0, 4.0));
    commands.spawn((node, shift, MenuLine, text("", &fonts.rockwell, 24.0, white), ChildOf(top_left)));
    let (node, shift) = anchored(align::CENTRE_X, Vec2::new(512.0, 4.0));
    commands.spawn((node, shift, TabHint, text("Press Tab for Menu", &fonts.rockwell, 24.0, white), Visibility::Hidden, ChildOf(top)));
    let (node, shift) = anchored(align::TOP_LEFT, Vec2::new(192.0, 4.0));
    commands.spawn((node, shift, SharesLine, text("", &fonts.rockwell, 24.0, white), ChildOf(top_left)));
    let (node, shift) = anchored(align::CENTRE_X, Vec2::new(512.0, 4.0));
    commands.spawn((node, shift, BudgetLine, text("", &fonts.rockwell, 24.0, white), ChildOf(top)));
}

pub(super) fn show_menu_line(
    net: Res<NetGame>,
    menu: Res<State<GameMenu>>,
    mut lines: ParamSet<(
        Query<(&mut Text, &mut Node, &mut Visibility), With<MenuLine>>,
        Query<(&mut Text, &mut Node, &mut Visibility), With<SharesLine>>,
        Query<(&mut Text, &mut Visibility), With<BudgetLine>>,
        Query<&mut Visibility, With<TabHint>>,
    )>,
) {
    let Some((p, _, _)) = own(&net) else { return };
    let open = *menu.get() == GameMenu::PlayerMenu;
    let world = net.sync.game_type == GameMode::World;
    let corporation = world && CORPORATION_MENUS.contains(&(p.menu as i32)) && p.team != Team::Spectator;
    let has_menu = p.menu != MenuType::Empty;

    let price = corporation_index(p.team).map(|t| net.share_prices[t]);
    let hint = has_menu && !open && !corporation;
    let wealth = has_menu && open && !world;

    let first = wealth.then(|| format!("${}", p.money));
    let second = if corporation { Some(format!("${}", p.team_money)) } else { price.filter(|_| wealth).map(|price| format!("${}", (price / 10).wrapping_mul(p.stocks))) };
    let third = corporation.then(|| format!("Budget: ${}", p.budget));

    for (mut t, mut node, mut v) in &mut lines.p0() {
        node.top = px(4.0);
        set_text(&mut t, &mut v, first.clone());
    }

    for (mut t, mut node, mut v) in &mut lines.p1() {
        node.left = px(if corporation { 196.0 } else { 192.0 });
        set_text(&mut t, &mut v, second.clone());
    }

    for (mut t, mut v) in &mut lines.p2() {
        set_text(&mut t, &mut v, third.clone());
    }

    for mut v in &mut lines.p3() {
        shown(&mut v, hint);
    }
}
