use bevy::prelude::*;

pub const LETTER_KEYS: [KeyCode; 26] = [
    KeyCode::KeyA, KeyCode::KeyB, KeyCode::KeyC, KeyCode::KeyD, KeyCode::KeyE, KeyCode::KeyF, KeyCode::KeyG, KeyCode::KeyH, KeyCode::KeyI,
    KeyCode::KeyJ, KeyCode::KeyK, KeyCode::KeyL, KeyCode::KeyM, KeyCode::KeyN, KeyCode::KeyO, KeyCode::KeyP, KeyCode::KeyQ, KeyCode::KeyR,
    KeyCode::KeyS, KeyCode::KeyT, KeyCode::KeyU, KeyCode::KeyV, KeyCode::KeyW, KeyCode::KeyX, KeyCode::KeyY, KeyCode::KeyZ,
];
pub const DIGIT_KEYS: [KeyCode; 10] =
    [KeyCode::Digit0, KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9];

pub fn mouse_button_bit(button: MouseButton) -> Option<u32> {
    match button {
        MouseButton::Left => Some(0),
        MouseButton::Middle => Some(1),
        MouseButton::Right => Some(2),
        MouseButton::Back => Some(3),
        MouseButton::Forward => Some(4),
        MouseButton::Other(n) if n < 16 => Some(n as u32),
        MouseButton::Other(_) => None,
    }
}

pub fn scancode(key: KeyCode) -> Option<i32> {
    if let Some(k) = LETTER_KEYS.iter().position(|&l| l == key) {
        return Some(4 + k as i32);
    }

    if let Some(k) = DIGIT_KEYS.iter().position(|&d| d == key) {
        return Some(30 + ((k + 9) % 10) as i32);
    }

    Some(match key {
        KeyCode::Enter => 40,
        KeyCode::Space => 44,
        KeyCode::Comma => 54,
        KeyCode::Period => 55,
        KeyCode::ArrowRight => 79,
        KeyCode::ArrowLeft => 80,
        KeyCode::ArrowDown => 81,
        KeyCode::ArrowUp => 82,
        KeyCode::Numpad7 => 95,
        KeyCode::ControlLeft => 224,
        KeyCode::ShiftLeft => 225,
        KeyCode::AltLeft => 226,
        KeyCode::Delete => 266,
        _ => return None,
    })
}

pub fn keycode(scancode: i32) -> Option<KeyCode> {
    if let 4..=29 = scancode {
        return LETTER_KEYS.get((scancode - 4) as usize).copied();
    }

    if let 30..=39 = scancode {
        return DIGIT_KEYS.get(((scancode - 30) as usize + 1) % 10).copied();
    }

    Some(match scancode {
        40 => KeyCode::Enter,
        44 => KeyCode::Space,
        54 => KeyCode::Comma,
        55 => KeyCode::Period,
        79 => KeyCode::ArrowRight,
        80 => KeyCode::ArrowLeft,
        81 => KeyCode::ArrowDown,
        82 => KeyCode::ArrowUp,
        95 => KeyCode::Numpad7,
        224 => KeyCode::ControlLeft,
        225 => KeyCode::ShiftLeft,
        226 => KeyCode::AltLeft,
        266 => KeyCode::Delete,
        _ => return None,
    })
}

pub fn key_name(scancode: i32) -> &'static str {
    const LETTERS: [&str; 26] = ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z"];
    const DIGITS: [&str; 10] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];

    match scancode {
        0 => "None",
        4..=29 => LETTERS[(scancode - 4) as usize],
        30..=39 => DIGITS[(scancode - 30) as usize],
        40 => "Enter",
        44 => "Space",
        54 => "Comma",
        55 => "Period",
        79 => "Right",
        80 => "Left",
        81 => "Down",
        82 => "Up",
        95 => "Home",
        224 => "Ctrl",
        225 => "Shift",
        226 => "Alt",
        256 => "Ins",
        257 => "End",
        259 => "PgDn",
        261 => "Center",
        265 => "PgUp",
        266 => "Del",
        269 => "Minus",
        270 => "Plus",
        _ => "",
    }
}
