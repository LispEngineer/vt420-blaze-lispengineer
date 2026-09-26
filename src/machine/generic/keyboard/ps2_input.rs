use crate::machine::generic::keyboard::ps2::Ps2Sender;
use crate::machine::generic::keyboard::{KeyCap, KeyboardInput};

/// Break prefix in scan code set 3.
const BREAK: u8 = 0xF0;

pub struct Ps2Input {
    sender: Ps2Sender,
    held: Vec<u8>,
}

impl Ps2Input {
    pub fn new(sender: Ps2Sender) -> Self {
        Self {
            sender,
            held: vec![],
        }
    }
}

impl KeyboardInput for Ps2Input {
    fn key_down(&mut self, key: KeyCap) {
        let Some(code) = set3_code(key) else {
            return;
        };
        if self.held.contains(&code) {
            return;
        }
        self.held.push(code);
        self.sender.send_raw(code);
    }

    fn key_up(&mut self, key: KeyCap) {
        let Some(code) = set3_code(key) else {
            return;
        };
        let Some(i) = self.held.iter().position(|&c| c == code) else {
            return;
        };
        self.held.remove(i);
        self.sender.send_raw(BREAK);
        self.sender.send_raw(code);
    }
}

/// Scan code set 3 make code for a key.
fn set3_code(key: KeyCap) -> Option<u8> {
    use KeyCap::*;
    Some(match key {
        Grave => 0x0E,
        Digit1 => 0x16,
        Digit2 => 0x1E,
        Digit3 => 0x26,
        Digit4 => 0x25,
        Digit5 => 0x2E,
        Digit6 => 0x36,
        Digit7 => 0x3D,
        Digit8 => 0x3E,
        Digit9 => 0x46,
        Digit0 => 0x45,
        Minus => 0x4E,
        Equal => 0x55,
        Backspace => 0x66,
        Tab => 0x0D,
        Q => 0x15,
        W => 0x1D,
        E => 0x24,
        R => 0x2D,
        T => 0x2C,
        Y => 0x35,
        U => 0x3C,
        I => 0x43,
        O => 0x44,
        P => 0x4D,
        LeftBracket => 0x54,
        RightBracket => 0x5B,
        Backslash => 0x5C,
        CapsLock => 0x14,
        A => 0x1C,
        S => 0x1B,
        D => 0x23,
        F => 0x2B,
        G => 0x34,
        H => 0x33,
        J => 0x3B,
        K => 0x42,
        L => 0x4B,
        Semicolon => 0x4C,
        Quote => 0x52,
        Enter => 0x5A,
        LeftShift => 0x12,
        LessGreater => 0x13,
        Z => 0x1A,
        X => 0x22,
        C => 0x21,
        V => 0x2A,
        B => 0x32,
        N => 0x31,
        M => 0x3A,
        Comma => 0x41,
        Period => 0x49,
        Slash => 0x4A,
        RightShift => 0x59,
        LeftCtrl => 0x11,
        LeftMeta => 0x8B,
        LeftAlt => 0x19,
        Space => 0x29,
        RightAlt => 0x39,
        RightMeta => 0x8C,
        Menu => 0x8D,
        RightCtrl => 0x58,
        Escape => 0x08,
        F1 => 0x07,
        F2 => 0x0F,
        F3 => 0x17,
        F4 => 0x1F,
        F5 => 0x27,
        F6 => 0x2F,
        F7 => 0x37,
        F8 => 0x3F,
        F9 => 0x47,
        F10 => 0x4F,
        F11 => 0x56,
        F12 => 0x5E,
        PrintScreen => 0x57,
        ScrollLock => 0x5F,
        Pause => 0x62,
        Insert => 0x67,
        Home => 0x6E,
        PageUp => 0x6F,
        Delete => 0x64,
        End => 0x65,
        PageDown => 0x6D,
        Up => 0x63,
        Down => 0x60,
        Left => 0x61,
        Right => 0x6A,
        NumLock => 0x76,
        KpDivide => 0x77,
        KpMultiply => 0x7E,
        KpMinus => 0x84,
        KpPlus => 0x7C,
        KpEnter => 0x79,
        KpPeriod => 0x71,
        Kp0 => 0x70,
        Kp1 => 0x69,
        Kp2 => 0x72,
        Kp3 => 0x7A,
        Kp4 => 0x6B,
        Kp5 => 0x73,
        Kp6 => 0x74,
        Kp7 => 0x6C,
        Kp8 => 0x75,
        Kp9 => 0x7D,
        Compose | F13 | F14 | Help | Do | F17 | F18 | F19 | F20 | Find | InsertHere | Remove
        | Select | PrevScreen | NextScreen | KpComma | KpPf1 | KpPf2 | KpPf3 | KpPf4 => {
            return None;
        }
    })
}
