//! The action map's key and mouse-button names (`data::input::KEYS`, Bevy's `KeyCode` variant names) ↔ Bevy's
//! codes.

use bevy::prelude::*;
use data::input::{Button, Mouse};

macro_rules! keys {
    ($($name:ident),* $(,)?) => {
        /// The code a key name stands for.
        pub fn key_code(name: &str) -> Option<KeyCode> {
            match name {
                $(stringify!($name) => Some(KeyCode::$name),)*
                _ => None,
            }
        }

        /// The name of a code, if the action map knows it.
        pub fn key_name(code: KeyCode) -> Option<&'static str> {
            match code {
                $(KeyCode::$name => data::input::KEYS.iter().copied().find(|k| *k == stringify!($name)),)*
                _ => None,
            }
        }
    };
}

keys! {
    KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO, KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV,
    KeyW, KeyX, KeyY, KeyZ, Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9, F1, F2, F3, F4, F5, F6, F7,
    F8, F9, F10, F11, F12, Numpad0, Numpad1, Numpad2, Numpad3, Numpad4, Numpad5, Numpad6, Numpad7, Numpad8, Numpad9, NumpadAdd,
    NumpadSubtract, NumpadMultiply, NumpadDivide, NumpadDecimal, NumpadEnter, ArrowUp, ArrowDown, ArrowLeft, ArrowRight, Space, Tab,
    Enter, Backspace, Delete, Insert, Home, End, PageUp, PageDown, Minus, Equal, BracketLeft, BracketRight, Backslash, Semicolon, Quote,
    Backquote, Comma, Period, Slash, CapsLock, ShiftLeft, ShiftRight, ControlLeft, ControlRight, AltLeft, AltRight,
}

pub fn mouse_button(m: Mouse) -> MouseButton {
    match m {
        Mouse::Left => MouseButton::Left,
        Mouse::Right => MouseButton::Right,
        Mouse::Middle => MouseButton::Middle,
        Mouse::Back => MouseButton::Back,
        Mouse::Forward => MouseButton::Forward,
    }
}

/// The action map's button for a mouse button, if it knows it.
pub fn mouse_of(b: MouseButton) -> Option<Mouse> {
    Mouse::ALL.into_iter().find(|m| mouse_button(*m) == b)
}

/// The action map's button for a key.
pub fn button_of(code: KeyCode) -> Option<Button> {
    key_name(code).map(Button::Key)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every name the action map allows has a code, and back.
    #[test]
    fn every_key_name_maps_both_ways() {
        for name in data::input::KEYS {
            let code = key_code(name).unwrap_or_else(|| panic!("{name} has no code"));
            assert_eq!(key_name(code), Some(*name));
        }
        for m in Mouse::ALL {
            assert_eq!(mouse_of(mouse_button(m)), Some(m));
        }
    }
}
