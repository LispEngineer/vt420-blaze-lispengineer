use tracing::{debug, trace};

/// I2C serial EEPROM (24C16-style)
pub struct I2cEeprom {
    pub mem: Vec<u8>,
    pub write_count: usize,

    phase: Phase,
    addr: u16,
    pull_low: bool,
    last_scl: bool,
    last_sda: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Idle,
    /// Shifting in a byte from the host.
    Receive {
        kind: Byte,
        bits: u8,
        byte: u8,
    },
    /// Driving ACK during the ninth clock.
    Ack {
        next: Next,
    },
    /// Shifting out a byte to the host.
    Transmit {
        bits: u8,
        byte: u8,
    },
    /// Ninth clock of a read, host ACK.
    HostAck {
        ack: Option<bool>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Byte {
    Control,
    WordAddress,
    Data,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Next {
    Receive(Byte),
    Transmit,
}

const SIZE: usize = 0x800;
const PAGE: u16 = 16;

impl Default for I2cEeprom {
    fn default() -> Self {
        Self::new()
    }
}

impl I2cEeprom {
    pub fn new() -> Self {
        Self {
            mem: vec![0xFF; SIZE],
            write_count: 0,
            phase: Phase::Idle,
            addr: 0,
            pull_low: false,
            last_scl: true,
            last_sda: true,
        }
    }

    pub fn sda(&self, host_sda: bool) -> bool {
        host_sda && !self.pull_low
    }

    pub fn update(&mut self, scl: bool, host_sda: bool) {
        let sda = self.sda(host_sda);
        if scl && self.last_scl && sda != self.last_sda {
            if !sda {
                trace!("EEPROM: START");
                self.phase = Phase::Receive {
                    kind: Byte::Control,
                    bits: 0,
                    byte: 0,
                };
            } else {
                trace!("EEPROM: STOP");
                self.phase = Phase::Idle;
            }
            self.pull_low = false;
        } else if scl && !self.last_scl {
            self.rising(sda);
        } else if !scl && self.last_scl {
            self.falling();
        }
        self.last_scl = scl;
        self.last_sda = self.sda(host_sda);
    }

    fn rising(&mut self, sda: bool) {
        match self.phase {
            Phase::Receive { kind, bits, byte } => {
                self.phase = Phase::Receive {
                    kind,
                    bits: bits + 1,
                    byte: byte << 1 | sda as u8,
                };
            }
            Phase::HostAck { .. } => self.phase = Phase::HostAck { ack: Some(!sda) },
            _ => {}
        }
    }

    fn falling(&mut self) {
        match self.phase {
            Phase::Receive {
                kind,
                bits: 8,
                byte,
            } => {
                if let Some(next) = self.received(kind, byte) {
                    self.phase = Phase::Ack { next };
                    self.pull_low = true;
                } else {
                    self.phase = Phase::Idle;
                }
            }
            Phase::Ack { next } => {
                self.pull_low = false;
                self.phase = match next {
                    Next::Receive(kind) => Phase::Receive {
                        kind,
                        bits: 0,
                        byte: 0,
                    },
                    Next::Transmit => Phase::Transmit {
                        bits: 0,
                        byte: self.next_read(),
                    },
                };
                self.drive_bit();
            }
            Phase::Transmit { bits, byte } => {
                if bits + 1 == 8 {
                    self.pull_low = false;
                    self.phase = Phase::HostAck { ack: None };
                } else {
                    self.phase = Phase::Transmit {
                        bits: bits + 1,
                        byte,
                    };
                    self.drive_bit();
                }
            }
            Phase::HostAck { ack: Some(true) } => {
                self.phase = Phase::Transmit {
                    bits: 0,
                    byte: self.next_read(),
                };
                self.drive_bit();
            }
            Phase::HostAck { ack: Some(false) } => self.phase = Phase::Idle,
            _ => {}
        }
    }

    fn drive_bit(&mut self) {
        if let Phase::Transmit { bits, byte } = self.phase {
            self.pull_low = byte & (0x80 >> bits) == 0;
        }
    }

    fn next_read(&mut self) -> u8 {
        let value = self.mem[self.addr as usize];
        trace!("EEPROM: read {:03X} = {value:02X}", self.addr);
        self.addr = (self.addr + 1) % SIZE as u16;
        value
    }

    fn received(&mut self, kind: Byte, byte: u8) -> Option<Next> {
        match kind {
            Byte::Control => {
                if byte & 0xF0 != 0xA0 {
                    debug!("I2C: control byte {byte:02X} for another device");
                    return None;
                }
                let block = ((byte >> 1) & 7) as u16;
                self.addr = block << 8 | (self.addr & 0xFF);
                if byte & 1 != 0 {
                    Some(Next::Transmit)
                } else {
                    Some(Next::Receive(Byte::WordAddress))
                }
            }
            Byte::WordAddress => {
                self.addr = (self.addr & 0x700) | byte as u16;
                Some(Next::Receive(Byte::Data))
            }
            Byte::Data => {
                trace!("EEPROM: write {:03X} = {byte:02X}", self.addr);
                self.mem[self.addr as usize] = byte;
                self.write_count += 1;
                self.addr = (self.addr & !(PAGE - 1)) | ((self.addr + 1) & (PAGE - 1));
                Some(Next::Receive(Byte::Data))
            }
        }
    }
}
