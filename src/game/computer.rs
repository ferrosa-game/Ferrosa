use bevy::{
    input::{ButtonState, keyboard::KeyboardInput},
    prelude::*,
};
use ferrosa_protocol::{
    clientbound::game::ItemKind,
    serverbound::game::actions::{GameAction, ItemAction},
};

use super::{escape_menu::GameMenu, netgame::NetGame};
use crate::menu::Screen;

const CTRL_FLAG: u32 = 0x100;
const NEW_LINE: u32 = 0xa;
const BACKSPACE: u32 = 8;
const SPACE: u32 = 0x20;
const ARROW_UP: u32 = 0x10;
const ARROW_DOWN: u32 = 0x11;
const ARROW_LEFT: u32 = 0x12;
const ARROW_RIGHT: u32 = 0x13;
const RIGHT_HAND: usize = 0;

#[derive(Resource, Default)]
pub struct AtComputer(pub Option<i32>);

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
        let &item = human.inventory[RIGHT_HAND].first()?;

        (usize::try_from(item).ok().and_then(|i| net.items.get(i))?.kind == Some(ItemKind::Computer)).then_some(item)
    });

    if at.0 != computer {
        at.0 = computer;
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
    const LETTERS: [KeyCode; 26] = [
        KeyCode::KeyA, KeyCode::KeyB, KeyCode::KeyC, KeyCode::KeyD, KeyCode::KeyE, KeyCode::KeyF, KeyCode::KeyG, KeyCode::KeyH, KeyCode::KeyI,
        KeyCode::KeyJ, KeyCode::KeyK, KeyCode::KeyL, KeyCode::KeyM, KeyCode::KeyN, KeyCode::KeyO, KeyCode::KeyP, KeyCode::KeyQ, KeyCode::KeyR,
        KeyCode::KeyS, KeyCode::KeyT, KeyCode::KeyU, KeyCode::KeyV, KeyCode::KeyW, KeyCode::KeyX, KeyCode::KeyY, KeyCode::KeyZ,
    ];
    const DIGITS: [KeyCode; 10] =
        [KeyCode::Digit0, KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9];

    if let Some(k) = LETTERS.iter().position(|&l| l == key) {
        return Some(b'a' + k as u8);
    }

    if let Some(k) = DIGITS.iter().position(|&d| d == key) {
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
            if ctrl { upper | CTRL_FLAG } else { upper }
        } else if shift {
            shifted(c) as u32
        } else {
            c as u32
        });
    }

    Some(match key {
        KeyCode::Space => SPACE,
        KeyCode::Enter | KeyCode::NumpadEnter => NEW_LINE,
        KeyCode::Backspace => BACKSPACE,
        KeyCode::ArrowUp => ARROW_UP,
        KeyCode::ArrowDown => ARROW_DOWN,
        KeyCode::ArrowLeft => ARROW_LEFT,
        KeyCode::ArrowRight => ARROW_RIGHT,
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
