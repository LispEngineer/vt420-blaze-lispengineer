use std::collections::VecDeque;

use tracing::debug;

#[derive(Default)]
pub struct Ps2Keyboard {
    rx: VecDeque<u8>,
    leds: u8,
    expect_arg: Option<u8>,
    pub pc_keyboard: bool,
}

impl Ps2Keyboard {
    pub fn command(&mut self, byte: u8) {
        if let Some(cmd) = self.expect_arg.take() {
            debug!("PS/2 command {cmd:02X} argument {byte:02X}");
            if cmd == 0xED {
                self.leds = byte;
                debug!("PS/2 LEDs = {byte:02X}");
            }
            self.rx.push_back(0xFA);
            return;
        }
        debug!("PS/2 command {byte:02X}");
        match byte {
            // Reset: ACK, then self-test passed.
            0xFF => self.rx.extend([0xFA, 0xAA]),
            // Identify: ACK, then an MF2 keyboard ID.
            0xF2 => self.rx.extend([0xFA, 0xAB, 0x83]),
            0xEE => self.rx.push_back(0xEE),
            0xAF if self.pc_keyboard => self.rx.push_back(0xFE),
            0xED | 0xF0 | 0xF3 => {
                self.expect_arg = Some(byte);
                self.rx.push_back(0xFA);
            }
            _ => self.rx.push_back(0xFA),
        }
    }

    pub fn leds(&self) -> u8 {
        self.leds
    }

    pub fn send(&mut self, bytes: &[u8]) {
        self.rx.extend(bytes);
    }

    pub fn has_data(&self) -> bool {
        !self.rx.is_empty()
    }

    pub fn pop(&mut self) -> Option<u8> {
        self.rx.pop_front()
    }
}
