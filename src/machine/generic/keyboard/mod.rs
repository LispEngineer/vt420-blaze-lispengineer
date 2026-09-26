pub mod lk201_input;
pub mod ps2;
pub mod ps2_input;

/// A physical key. Union of PC 101/102-key and a DEC LK201 key caps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyCap {
    Grave,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    Digit0,
    Minus,
    Equal,
    Backspace,
    Tab,
    Q,
    W,
    E,
    R,
    T,
    Y,
    U,
    I,
    O,
    P,
    LeftBracket,
    RightBracket,
    Backslash,
    CapsLock,
    A,
    S,
    D,
    F,
    G,
    H,
    J,
    K,
    L,
    Semicolon,
    Quote,
    Enter,
    LeftShift,
    /// The extra key left of Z on ISO and LK201 keyboards (`<` `>`).
    LessGreater,
    Z,
    X,
    C,
    V,
    B,
    N,
    M,
    Comma,
    Period,
    Slash,
    RightShift,
    LeftCtrl,
    LeftMeta,
    LeftAlt,
    Space,
    RightAlt,
    RightMeta,
    Menu,
    RightCtrl,
    /// LK201 Compose Character.
    Compose,

    Escape,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    /// LK201 Help (F15 position).
    Help,
    /// LK201 Do (F16 position).
    Do,
    F17,
    F18,
    F19,
    F20,

    PrintScreen,
    ScrollLock,
    Pause,

    Insert,
    Home,
    PageUp,
    Delete,
    End,
    PageDown,
    Find,
    InsertHere,
    Remove,
    Select,
    PrevScreen,
    NextScreen,

    Up,
    Down,
    Left,
    Right,

    NumLock,
    KpDivide,
    KpMultiply,
    KpMinus,
    KpPlus,
    KpEnter,
    KpPeriod,
    KpComma,
    Kp0,
    Kp1,
    Kp2,
    Kp3,
    Kp4,
    Kp5,
    Kp6,
    Kp7,
    Kp8,
    Kp9,
    KpPf1,
    KpPf2,
    KpPf3,
    KpPf4,
}

/// A keyboard that takes physical key presses and releases.
pub trait KeyboardInput {
    fn key_down(&mut self, key: KeyCap);

    fn key_up(&mut self, key: KeyCap);

    /// Press and release one key.
    fn tap(&mut self, key: KeyCap) {
        self.key_down(key);
        self.key_up(key);
    }

    /// Press the keys in order, then release them in reverse order.
    fn chord(&mut self, keys: &[KeyCap]) {
        for &key in keys {
            self.key_down(key);
        }
        for &key in keys.iter().rev() {
            self.key_up(key);
        }
    }
}
