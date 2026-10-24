use bevy::{
    input::{ButtonState, keyboard::KeyboardInput},
    prelude::*,
};
use ferrosa_protocol::{
    clientbound::game::ItemKind,
    serverbound::game::actions::{GameAction, ItemAction},
};

use super::{escape_menu::GameMenu, netgame::NetGame};
use crate::{
    keys::{DIGIT_KEYS, LETTER_KEYS},
    menu::Screen,
};

#[derive(Resource, Default)]
pub struct AtComputer(pub Option<i32>, Option<i32>);

impl AtComputer {
    pub fn typing(&self) -> bool {
        self.0.is_some()
    }
}

pub struct ComputerPlugin;

impl Plugin for ComputerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AtComputer>()
            .add_systems(PreUpdate, find_computer.run_if(in_state(Screen::InGame)))
            .add_systems(Update, type_on_computer.run_if(in_state(Screen::InGame)).run_if(resource_exists::<NetGame>))
            .add_systems(OnExit(Screen::InGame), |mut at: ResMut<AtComputer>| at.0 = None);
    }
}

fn find_computer(net: Option<Res<NetGame>>, mouse: Res<ButtonInput<MouseButton>>, menu: Option<Res<State<GameMenu>>>, mut at: ResMut<AtComputer>) {
    let playing = menu.is_some_and(|m| *m.get() == GameMenu::Closed);
    let computer = net.filter(|_| playing && !mouse.pressed(MouseButton::Right)).and_then(|net| {
        let p = net.me()?;
        let human = usize::try_from(p.human).ok().and_then(|h| net.humans.get(h)).filter(|h| h.alive)?;
        let &item = human.inventory[0].first()?;

        (usize::try_from(item).ok().and_then(|i| net.items.get(i))?.kind == Some(ItemKind::Computer)).then_some(item)
    });

    if at.1.is_some() && at.1 != computer {
        at.1 = None;
    }

    if computer.is_some() && mouse.just_pressed(MouseButton::Left) {
        at.1 = if at.1 == computer { None } else { computer };
    }

    let typing = computer.filter(|_| at.1 != computer);

    if at.0 != typing {
        at.0 = typing;
    }
}

fn shifted(c: u8) -> u8 {
    match c {
        b'\'' => b'"',
        b',' => b'<',
        b'-' => b'_',
        b'.' => b'>',
        b'0' => b')',
        b'1' => b'!',
        b'2' => b'@',
        b'3' => b'#',
        b'4' => b'$',
        b'5' => b'%',
        b'6' => b'^',
        b'7' => b'&',
        b'8' => b'*',
        b'9' => b'(',
        b';' => b':',
        b'=' => b'+',
        b'[' => b'{',
        b'\\' => b'|',
        b']' => b'}',
        _ => b'?',
    }
}

fn character(key: KeyCode) -> Option<u8> {
    if let Some(k) = LETTER_KEYS.iter().position(|&l| l == key) {
        return Some(b'a' + k as u8);
    }

    if let Some(k) = DIGIT_KEYS.iter().position(|&d| d == key) {
        return Some(b'0' + k as u8);
    }

    Some(match key {
        KeyCode::Quote => b'\'',
        KeyCode::Comma => b',',
        KeyCode::Minus => b'-',
        KeyCode::Period => b'.',
        KeyCode::Slash => b'/',
        KeyCode::Semicolon => b';',
        KeyCode::Equal => b'=',
        KeyCode::BracketLeft => b'[',
        KeyCode::Backslash => b'\\',
        KeyCode::BracketRight => b']',
        _ => return None,
    })
}

fn computer_key(key: KeyCode, shift: bool, ctrl: bool) -> Option<u32> {
    if let Some(c) = character(key) {
        return Some(if c.is_ascii_lowercase() {
            let upper = c.to_ascii_uppercase() as u32;
            if ctrl { upper | 256 } else { upper }
        } else if shift {
            shifted(c) as u32
        } else {
            c as u32
        });
    }

    Some(match key {
        KeyCode::Space => 32,
        KeyCode::Enter | KeyCode::NumpadEnter => 10,
        KeyCode::Backspace => 8,
        KeyCode::ArrowUp => 16,
        KeyCode::ArrowDown => 17,
        KeyCode::ArrowLeft => 18,
        KeyCode::ArrowRight => 19,
        _ => return None,
    })
}

fn type_on_computer(mut typed: MessageReader<KeyboardInput>, keys: Res<ButtonInput<KeyCode>>, at: Res<AtComputer>, mut net: ResMut<NetGame>) {
    let Some(item) = at.0 else {
        typed.clear();
        return;
    };

    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    let codes: Vec<u32> = typed.read().filter(|k| k.state == ButtonState::Pressed).filter_map(|k| computer_key(k.key_code, shift, ctrl)).collect();
    let Some(p) = net.me_mut() else { return };

    for key in codes {
        p.queue(GameAction::Item(ItemAction { a: item as u16, b: key as u16 }));
    }
}
