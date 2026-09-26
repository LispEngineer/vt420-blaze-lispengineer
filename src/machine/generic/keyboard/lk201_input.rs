use lk201::{Key, LK201Sender, SpecialKey};

use crate::machine::generic::keyboard::{KeyCap, KeyboardInput};

/// Sent when the last held up/down key is released.
const ALL_UP: u8 = 0xB3;

pub struct Lk201Input {
    sender: LK201Sender,
    held: Vec<u8>,
}

impl Lk201Input {
    pub fn new(sender: LK201Sender) -> Self {
        Self {
            sender,
            held: vec![],
        }
    }

    fn send(&self, code: u8) {
        self.sender.send_raw(code);
    }

    fn down(&mut self, code: u8) {
        if is_up_down(code) {
            if self.held.contains(&code) {
                return;
            }
            self.held.push(code);
        }
        self.send(code);
    }

    fn up(&mut self, code: u8) {
        if !is_up_down(code) {
            return;
        }
        let Some(i) = self.held.iter().position(|&c| c == code) else {
            return;
        };
        self.held.remove(i);
        self.send(if self.held.is_empty() { ALL_UP } else { code });
    }
}

impl KeyboardInput for Lk201Input {
    fn key_down(&mut self, key: KeyCap) {
        // The LK201 has no Escape key: send Ctrl-3.
        if key == KeyCap::Escape {
            self.down(SpecialKey::Ctrl as u8);
            self.down(char_code('3'));
            return;
        }
        if let Some(code) = lk201_code(key) {
            self.down(code);
        }
    }

    fn key_up(&mut self, key: KeyCap) {
        if key == KeyCap::Escape {
            self.up(SpecialKey::Ctrl as u8);
            return;
        }
        if let Some(code) = lk201_code(key) {
            self.up(code);
        }
    }
}

fn is_up_down(code: u8) -> bool {
    use SpecialKey::*;
    [Shift, RShift, Ctrl, Lock, Meta, F1, F2, F3, F4, F5]
        .iter()
        .any(|&k| k as u8 == code)
}

fn char_code(c: char) -> u8 {
    Key::char_to_keycode(c).map(|(code, _)| code).unwrap()
}

fn lk201_code(key: KeyCap) -> Option<u8> {
    use KeyCap::*;
    let c = match key {
        Grave => '`',
        Digit1 => '1',
        Digit2 => '2',
        Digit3 => '3',
        Digit4 => '4',
        Digit5 => '5',
        Digit6 => '6',
        Digit7 => '7',
        Digit8 => '8',
        Digit9 => '9',
        Digit0 => '0',
        Minus => '-',
        Equal => '=',
        Q => 'q',
        W => 'w',
        E => 'e',
        R => 'r',
        T => 't',
        Y => 'y',
        U => 'u',
        I => 'i',
        O => 'o',
        P => 'p',
        LeftBracket => '[',
        RightBracket => ']',
        Backslash => '\\',
        A => 'a',
        S => 's',
        D => 'd',
        F => 'f',
        G => 'g',
        H => 'h',
        J => 'j',
        K => 'k',
        L => 'l',
        Semicolon => ';',
        Quote => '\'',
        LessGreater => '<',
        Z => 'z',
        X => 'x',
        C => 'c',
        V => 'v',
        B => 'b',
        N => 'n',
        M => 'm',
        Comma => ',',
        Period => '.',
        Slash => '/',
        Space => ' ',
        _ => return special_key(key).map(|k| k as u8),
    };
    Some(char_code(c))
}

fn special_key(key: KeyCap) -> Option<SpecialKey> {
    use KeyCap::*;
    Some(match key {
        Backspace => SpecialKey::Delete,
        Tab => SpecialKey::Tab,
        CapsLock => SpecialKey::Lock,
        Enter => SpecialKey::Return,
        LeftShift => SpecialKey::Shift,
        RightShift => SpecialKey::RShift,
        LeftCtrl | RightCtrl => SpecialKey::Ctrl,
        Compose => SpecialKey::Meta,
        F1 => SpecialKey::F1,
        F2 => SpecialKey::F2,
        F3 => SpecialKey::F3,
        F4 => SpecialKey::F4,
        F5 => SpecialKey::F5,
        F6 => SpecialKey::F6,
        F7 => SpecialKey::F7,
        F8 => SpecialKey::F8,
        F9 => SpecialKey::F9,
        F10 => SpecialKey::F10,
        F11 => SpecialKey::F11,
        F12 => SpecialKey::F12,
        F13 => SpecialKey::F13,
        F14 => SpecialKey::F14,
        Help => SpecialKey::Help,
        Do => SpecialKey::Menu,
        F17 => SpecialKey::F17,
        F18 => SpecialKey::F18,
        F19 => SpecialKey::F19,
        F20 => SpecialKey::F20,
        Find | Home => SpecialKey::Find,
        InsertHere | Insert => SpecialKey::InsertHere,
        Remove | Delete => SpecialKey::Remove,
        Select | End => SpecialKey::Select,
        PrevScreen | PageUp => SpecialKey::PrevScreen,
        NextScreen | PageDown => SpecialKey::NextScreen,
        Up => SpecialKey::Up,
        Down => SpecialKey::Down,
        Left => SpecialKey::Left,
        Right => SpecialKey::Right,
        KpPf1 | NumLock => SpecialKey::KpPf1,
        KpPf2 | KpDivide => SpecialKey::KpPf2,
        KpPf3 | KpMultiply => SpecialKey::KpPf3,
        KpPf4 => SpecialKey::KpPf4,
        KpMinus => SpecialKey::KpHyphen,
        KpComma | KpPlus => SpecialKey::KpComma,
        KpEnter => SpecialKey::KpEnter,
        KpPeriod => SpecialKey::KpPeriod,
        Kp0 => SpecialKey::Kp0,
        Kp1 => SpecialKey::Kp1,
        Kp2 => SpecialKey::Kp2,
        Kp3 => SpecialKey::Kp3,
        Kp4 => SpecialKey::Kp4,
        Kp5 => SpecialKey::Kp5,
        Kp6 => SpecialKey::Kp6,
        Kp7 => SpecialKey::Kp7,
        Kp8 => SpecialKey::Kp8,
        Kp9 => SpecialKey::Kp9,
        _ => return None,
    })
}
