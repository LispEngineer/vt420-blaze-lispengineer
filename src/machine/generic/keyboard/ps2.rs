use std::collections::VecDeque;
use std::sync::mpsc;

use tracing::debug;

pub struct Ps2Sender {
    send: mpsc::Sender<u8>,
}

impl Ps2Sender {
    pub fn send_raw(&self, code: u8) {
        _ = self.send.send(code);
    }
}

pub struct Ps2Keyboard {
    rx: VecDeque<u8>,
    leds: u8,
    expect_arg: Option<u8>,
    pub pc_keyboard: bool,
    pub present: bool,
    /// Scanning is off after 0xF5 (disable) until 0xF4 (enable) or a reset.
    enabled: bool,
    scan_set: u8,
    send: mpsc::Sender<u8>,
    recv: mpsc::Receiver<u8>,
}

impl Default for Ps2Keyboard {
    fn default() -> Self {
        let (send, recv) = mpsc::channel();
        Self {
            rx: VecDeque::new(),
            leds: 0,
            expect_arg: None,
            pc_keyboard: false,
            present: true,
            enabled: true,
            scan_set: 2,
            send,
            recv,
        }
    }
}

impl Ps2Keyboard {
    pub fn sender(&self) -> Ps2Sender {
        Ps2Sender {
            send: self.send.clone(),
        }
    }

    /// Move key bytes from the senders into the receive queue.
    pub fn tick(&mut self) {
        while let Ok(byte) = self.recv.try_recv() {
            if self.enabled {
                self.rx.push_back(byte);
            }
        }
    }

    pub fn command(&mut self, byte: u8) {
        if !self.present {
            return;
        }
        if let Some(cmd) = self.expect_arg.take() {
            debug!("PS/2 command {cmd:02X} argument {byte:02X}");
            match cmd {
                0xED => {
                    self.leds = byte;
                    debug!("PS/2 LEDs = {byte:02X}");
                }
                // Scan code set, 0 for query.
                0xF0 if byte == 0 => {
                    self.rx.extend([0xFA, self.scan_set]);
                    return;
                }
                0xF0 => self.scan_set = byte,
                _ => {}
            }
            self.rx.push_back(0xFA);
            return;
        }
        debug!("PS/2 command {byte:02X}");
        match byte {
            // Reset: ACK, then self-test passed.
            0xFF => {
                self.enabled = true;
                self.scan_set = 2;
                self.rx.extend([0xFA, 0xAA]);
            }
            0xF4 => {
                self.enabled = true;
                self.rx.push_back(0xFA);
            }
            0xF5 => {
                self.enabled = false;
                self.rx.push_back(0xFA);
            }
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
