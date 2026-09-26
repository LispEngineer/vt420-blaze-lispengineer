use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::machine::generic::keyboard::{KeyCap, KeyboardInput};

#[derive(Default)]
pub struct CrosstermKeyboard {
    compose_special_key: bool,
}

pub enum KeyboardCommand {
    ToggleRun,
    ToggleHexMode,
    DumpVRAM,
    /// Ctrl-G then `p`: flush `--pc-trace` file immediately
    #[cfg(all(feature = "pc-trace", not(target_arch = "wasm32")))]
    FlushPCTrace,
    Quit,
}

impl CrosstermKeyboard {
    pub fn update_keyboard(
        &mut self,
        event: &Event,
        keyboard: &mut dyn KeyboardInput,
    ) -> Option<KeyboardCommand> {
        let Event::Key(key) = event else {
            return None;
        };
        if key.kind == KeyEventKind::Release {
            return None;
        }
        if self.compose_special_key {
            self.compose_special_key = false;
            if key.modifiers.is_empty() {
                match key.code {
                    KeyCode::Char('1') => keyboard.tap(KeyCap::F1),
                    KeyCode::Char('2') => keyboard.tap(KeyCap::F2),
                    KeyCode::Char('3') => keyboard.tap(KeyCap::F3),
                    KeyCode::Char('4') => keyboard.tap(KeyCap::F4),
                    KeyCode::Char('5') => keyboard.tap(KeyCap::F5),
                    KeyCode::Char('c') => keyboard.tap(KeyCap::CapsLock),
                    KeyCode::Char('q') => return Some(KeyboardCommand::Quit),
                    KeyCode::Char(' ') => return Some(KeyboardCommand::ToggleRun),
                    KeyCode::Char('h') => return Some(KeyboardCommand::ToggleHexMode),
                    KeyCode::Char('d') => return Some(KeyboardCommand::DumpVRAM),
                    #[cfg(all(feature = "pc-trace", not(target_arch = "wasm32")))]
                    KeyCode::Char('p') => return Some(KeyboardCommand::FlushPCTrace),
                    _ => {}
                }
            }
            return None;
        }
        if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('g') {
            self.compose_special_key = true;
            return None;
        }
        send_key(key, keyboard);
        None
    }
}

pub fn send_key(key: &KeyEvent, keyboard: &mut dyn KeyboardInput) {
    keyboard.chord(&key_chord(key));
}

pub fn key_chord(key: &KeyEvent) -> Vec<KeyCap> {
    let (cap, mut shift) = match key.code {
        KeyCode::Char(c) => match char_cap(c) {
            Some(found) => found,
            None => return vec![],
        },
        KeyCode::BackTab => (KeyCap::Tab, true),
        code => match key_cap(code) {
            Some(cap) => (cap, false),
            None => return vec![],
        },
    };
    shift |= key.modifiers.contains(KeyModifiers::SHIFT);
    let mut chord = vec![];
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        chord.push(KeyCap::LeftCtrl);
    }
    if key.modifiers.contains(KeyModifiers::ALT) {
        chord.push(KeyCap::LeftAlt);
    }
    if shift {
        chord.push(KeyCap::LeftShift);
    }
    chord.push(cap);
    chord
}

fn key_cap(code: KeyCode) -> Option<KeyCap> {
    Some(match code {
        KeyCode::Esc => KeyCap::Escape,
        KeyCode::Enter => KeyCap::Enter,
        KeyCode::Backspace => KeyCap::Backspace,
        KeyCode::Tab => KeyCap::Tab,
        KeyCode::Up => KeyCap::Up,
        KeyCode::Down => KeyCap::Down,
        KeyCode::Left => KeyCap::Left,
        KeyCode::Right => KeyCap::Right,
        KeyCode::Insert => KeyCap::Insert,
        KeyCode::Delete => KeyCap::Delete,
        KeyCode::Home => KeyCap::Home,
        KeyCode::End => KeyCap::End,
        KeyCode::PageUp => KeyCap::PageUp,
        KeyCode::PageDown => KeyCap::PageDown,
        KeyCode::CapsLock => KeyCap::CapsLock,
        KeyCode::ScrollLock => KeyCap::ScrollLock,
        KeyCode::NumLock => KeyCap::NumLock,
        KeyCode::PrintScreen => KeyCap::PrintScreen,
        KeyCode::Pause => KeyCap::Pause,
        KeyCode::Menu => KeyCap::Menu,
        KeyCode::F(n) => {
            const F: [KeyCap; 20] = [
                KeyCap::F1,
                KeyCap::F2,
                KeyCap::F3,
                KeyCap::F4,
                KeyCap::F5,
                KeyCap::F6,
                KeyCap::F7,
                KeyCap::F8,
                KeyCap::F9,
                KeyCap::F10,
                KeyCap::F11,
                KeyCap::F12,
                KeyCap::F13,
                KeyCap::F14,
                KeyCap::Help,
                KeyCap::Do,
                KeyCap::F17,
                KeyCap::F18,
                KeyCap::F19,
                KeyCap::F20,
            ];
            *F.get((n as usize).checked_sub(1)?)?
        }
        _ => return None,
    })
}

fn char_cap(c: char) -> Option<(KeyCap, bool)> {
    const ROWS: [(&str, &str, &[KeyCap]); 4] = [
        (
            "`1234567890-=",
            "~!@#$%^&*()_+",
            &[
                KeyCap::Grave,
                KeyCap::Digit1,
                KeyCap::Digit2,
                KeyCap::Digit3,
                KeyCap::Digit4,
                KeyCap::Digit5,
                KeyCap::Digit6,
                KeyCap::Digit7,
                KeyCap::Digit8,
                KeyCap::Digit9,
                KeyCap::Digit0,
                KeyCap::Minus,
                KeyCap::Equal,
            ],
        ),
        (
            "qwertyuiop[]\\",
            "QWERTYUIOP{}|",
            &[
                KeyCap::Q,
                KeyCap::W,
                KeyCap::E,
                KeyCap::R,
                KeyCap::T,
                KeyCap::Y,
                KeyCap::U,
                KeyCap::I,
                KeyCap::O,
                KeyCap::P,
                KeyCap::LeftBracket,
                KeyCap::RightBracket,
                KeyCap::Backslash,
            ],
        ),
        (
            "asdfghjkl;'",
            "ASDFGHJKL:\"",
            &[
                KeyCap::A,
                KeyCap::S,
                KeyCap::D,
                KeyCap::F,
                KeyCap::G,
                KeyCap::H,
                KeyCap::J,
                KeyCap::K,
                KeyCap::L,
                KeyCap::Semicolon,
                KeyCap::Quote,
            ],
        ),
        (
            "zxcvbnm,./",
            "ZXCVBNM<>?",
            &[
                KeyCap::Z,
                KeyCap::X,
                KeyCap::C,
                KeyCap::V,
                KeyCap::B,
                KeyCap::N,
                KeyCap::M,
                KeyCap::Comma,
                KeyCap::Period,
                KeyCap::Slash,
            ],
        ),
    ];
    if c == ' ' {
        return Some((KeyCap::Space, false));
    }
    for (plain, shifted, caps) in ROWS {
        if let Some(i) = plain.chars().position(|p| p == c) {
            return Some((caps[i], false));
        }
        if let Some(i) = shifted.chars().position(|p| p == c) {
            return Some((caps[i], true));
        }
    }
    None
}
