//! The LK250: an LK201-layout keyboard with a PC interface.

use std::collections::VecDeque;

use i8051::peripheral::{Serial, Timer, TimerTick};
use i8051::sfr::{SFR_P0, SFR_P1, SFR_P2, SFR_P3};
use i8051::{Cpu, CpuContext, CpuView, PortMapper, ReadOnlyMemoryMapper};

use super::lk201::{ScanCell, matrix_row};

static LK250_ROM: &[u8] = include_bytes!("23-058M1.BIN");

const ROW_COUNT: usize = 18;

/// P3.0 reads the data line and P3.2 the clock line (1 = low).
const P3_DATA_IN: u8 = 1 << 0;
const P3_CLOCK_IN: u8 = 1 << 2;
/// P3.1 and P3.3 drive the data and clock lines through inverters (1 = low).
const P3_DATA_OUT: u8 = 1 << 1;
const P3_CLOCK_OUT: u8 = 1 << 3;
/// P3.5 is the mode strap, read with P3.6 low: low selects AT, high XT.
const P3_STRAP: u8 = 1 << 5;
/// P3.7 low enters a test mode at power-up.
const P3_TEST: u8 = 1 << 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lk250Mode {
    Xt,
    At,
}

struct LK250Ports {
    latch: [u8; 4],
    p0: u8,
    key_matrix: [u8; ROW_COUNT],
    mode: Lk250Mode,
    host_clock_low: bool,
    host_data_low: bool,
}

impl LK250Ports {
    fn clock_line(&self) -> bool {
        self.latch[3] & P3_CLOCK_OUT == 0 && !self.host_clock_low
    }

    fn data_line(&self) -> bool {
        self.latch[3] & P3_DATA_OUT == 0 && !self.host_data_low
    }
}

impl PortMapper for LK250Ports {
    type WriteValue = (u8, u8);
    fn interest<C: CpuView>(&self, _cpu: &C, addr: u8) -> bool {
        addr == SFR_P0 || addr == SFR_P1 || addr == SFR_P2 || addr == SFR_P3
    }
    fn read<C: CpuView>(&self, _cpu: &C, addr: u8) -> u8 {
        match addr {
            SFR_P0 => self.p0,
            SFR_P1 => self.latch[1],
            SFR_P2 => self.latch[2],
            SFR_P3 => {
                let mut p3 = self.latch[3] & !(P3_DATA_IN | P3_CLOCK_IN | P3_STRAP) | P3_TEST;
                if !self.data_line() {
                    p3 |= P3_DATA_IN;
                }
                if !self.clock_line() {
                    p3 |= P3_CLOCK_IN;
                }
                if self.mode == Lk250Mode::Xt {
                    p3 |= P3_STRAP;
                }
                p3
            }
            _ => unreachable!(),
        }
    }
    fn prepare_write<C: CpuView>(&self, _cpu: &C, addr: u8, value: u8) -> Self::WriteValue {
        (addr, value)
    }
    fn write(&mut self, (addr, value): Self::WriteValue) {
        match addr {
            SFR_P0 => self.latch[0] = value,
            SFR_P1 => {
                self.latch[1] = value;
                self.p0 = match matrix_row(value) {
                    Some(row) => !self.key_matrix[row],
                    None => 0xFF,
                };
            }
            SFR_P2 => self.latch[2] = value,
            SFR_P3 => self.latch[3] = value,
            _ => unreachable!(),
        }
    }
    fn read_latch<C: CpuView>(&self, _cpu: &C, addr: u8) -> u8 {
        match addr {
            SFR_P0 => self.latch[0],
            SFR_P1 => self.latch[1],
            SFR_P2 => self.latch[2],
            SFR_P3 => self.latch[3],
            _ => unreachable!(),
        }
    }
}

struct LK250System {
    ports: LK250Ports,
    serial: Serial,
    timer: Timer,
}

impl PortMapper for LK250System {
    type WriteValue = <(LK250Ports, (Serial, Timer)) as PortMapper>::WriteValue;
    fn interest<C: CpuView>(&self, cpu: &C, addr: u8) -> bool {
        self.ports.interest(cpu, addr)
            || self.serial.interest(cpu, addr)
            || self.timer.interest(cpu, addr)
    }
    fn read<C: CpuView>(&self, cpu: &C, addr: u8) -> u8 {
        (&self.ports, (&self.serial, &self.timer)).read(cpu, addr)
    }
    fn prepare_write<C: CpuView>(&self, cpu: &C, addr: u8, value: u8) -> Self::WriteValue {
        (&self.ports, (&self.serial, &self.timer)).prepare_write(cpu, addr, value)
    }
    fn write(&mut self, write: Self::WriteValue) {
        (&mut self.ports, (&mut self.serial, &mut self.timer)).write(write)
    }
    fn read_latch<C: CpuView>(&self, cpu: &C, addr: u8) -> u8 {
        (&self.ports, (&self.serial, &self.timer)).read_latch(cpu, addr)
    }
}

impl ReadOnlyMemoryMapper for LK250System {
    fn len(&self) -> u32 {
        LK250_ROM.len() as u32
    }
    fn read<C: CpuView>(&self, _cpu: &C, addr: u32) -> u8 {
        LK250_ROM.get(addr as usize).copied().unwrap_or(0xFF)
    }
}

impl CpuContext for LK250System {
    type Ports = LK250System;
    type Xdata = ();
    type Code = LK250System;

    fn ports(&self) -> &Self::Ports {
        self
    }
    fn ports_mut(&mut self) -> &mut Self::Ports {
        self
    }
    fn xdata(&self) -> &Self::Xdata {
        unreachable!()
    }
    fn xdata_mut(&mut self) -> &mut Self::Xdata {
        unreachable!()
    }
    fn code(&self) -> &Self::Code {
        self
    }
    fn code_mut(&mut self) -> &mut Self::Code {
        self
    }
}

/// Machine cycles for an 8051 instruction, by opcode. i8051 doesn't model this
/// yet.
fn machine_cycles(op: u8) -> usize {
    match op {
        0xA4 | 0x84 => 4,
        op if op & 0x1F == 0x01 => 2,
        0x02 | 0x12 | 0x22 | 0x32 | 0x80 | 0x73 => 2,
        0x10 | 0x20 | 0x30 | 0x40 | 0x50 | 0x60 | 0x70 => 2,
        0xB4..=0xBF | 0xD5 | 0xD8..=0xDF => 2,
        0x83 | 0x93 | 0xE0 | 0xE2 | 0xE3 | 0xF0 | 0xF2 | 0xF3 => 2,
        0x90 | 0xA3 | 0x75 | 0x85 | 0x86 | 0x87 | 0x88..=0x8F => 2,
        0xA6 | 0xA7 | 0xA8..=0xAF | 0xC0 | 0xD0 => 2,
        0x43 | 0x53 | 0x63 | 0x72 | 0xA0 | 0x82 | 0xB0 | 0x92 => 2,
        _ => 1,
    }
}

/// Instructions the clock stays high after a frame before it is complete.
const FRAME_GAP: usize = 200;
/// Instructions the host holds the clock low before sending (AT).
const HOST_INHIBIT: usize = 100;

enum HostSend {
    Idle,
    Inhibit { ticks: usize, bits: Vec<bool> },
    Bits { bits: Vec<bool>, next: usize },
}

/// A hardware simulator for the LK250 keyboard. Reads bit-banged keys.
pub struct LK250Hardware {
    cpu: Cpu,
    system: LK250System,
    mode: Lk250Mode,
    last_clock: bool,
    idle: usize,
    bits: Vec<bool>,
    received: VecDeque<u8>,
    host: HostSend,
    pending: VecDeque<u8>,
}

impl LK250Hardware {
    pub fn new(mode: Lk250Mode) -> Self {
        Self {
            cpu: Cpu::new(),
            system: LK250System {
                ports: LK250Ports {
                    latch: [0xFF; 4],
                    p0: 0xFF,
                    key_matrix: [0; ROW_COUNT],
                    mode,
                    host_clock_low: false,
                    host_data_low: false,
                },
                serial: Serial::new(60).0,
                timer: Timer::default(),
            },
            mode,
            last_clock: true,
            idle: 0,
            bits: vec![],
            received: VecDeque::new(),
            host: HostSend::Idle,
            pending: VecDeque::new(),
        }
    }

    pub fn press_key(&mut self, cell: ScanCell) {
        self.system.ports.key_matrix[cell.row()] |= 1 << cell.col();
    }

    pub fn release_key(&mut self, cell: ScanCell) {
        self.system.ports.key_matrix[cell.row()] &= !(1 << cell.col());
    }

    pub fn release_all_keys(&mut self) {
        self.system.ports.key_matrix.fill(0);
    }

    /// The LED outputs: P2, active low.
    pub fn leds(&self) -> u8 {
        self.system.ports.latch[2]
    }

    /// Receive a byte.
    pub fn recv(&mut self) -> Option<u8> {
        self.received.pop_front()
    }

    /// Send a command byte to the keyboard (AT mode).
    pub fn send(&mut self, byte: u8) {
        self.pending.push_back(byte);
    }

    /// Hold the clock line low. In XT mode this resets the keyboard.
    pub fn hold_clock_low(&mut self, low: bool) {
        self.system.ports.host_clock_low = low;
    }

    pub fn tick(&mut self) {
        let op = LK250_ROM
            .get(self.cpu.pc_ext(&self.system) as usize)
            .copied()
            .unwrap_or(0);
        self.cpu.step(&mut self.system);
        for _ in 0..machine_cycles(op) {
            let tick = self.system.timer.prepare_tick(&mut self.cpu, &self.system);
            self.system.timer.tick(&mut self.cpu, tick);
        }
        self.watch_lines();
    }

    fn watch_lines(&mut self) {
        let clock = self.system.ports.clock_line();
        let rising = clock && !self.last_clock;
        let falling = !clock && self.last_clock;
        self.last_clock = clock;

        if let HostSend::Bits { bits, next } = &mut self.host {
            if falling {
                if let Some(&bit) = bits.get(*next) {
                    self.system.ports.host_data_low = !bit;
                    *next += 1;
                } else {
                    self.system.ports.host_data_low = false;
                    self.host = HostSend::Idle;
                }
            }
            return;
        }
        if let HostSend::Inhibit { ticks, bits } = &mut self.host {
            *ticks -= 1;
            if *ticks == 0 {
                self.system.ports.host_clock_low = false;
                self.host = HostSend::Bits {
                    bits: std::mem::take(bits),
                    next: 0,
                };
            }
            return;
        }

        if rising {
            self.bits.push(self.system.ports.data_line());
            self.idle = 0;
        } else if clock {
            self.idle += 1;
            if self.idle == FRAME_GAP {
                self.decode_frames();
            }
            if self.idle >= FRAME_GAP && self.mode == Lk250Mode::At {
                if let Some(byte) = self.pending.pop_front() {
                    self.start_send(byte);
                }
            }
        }
    }

    /// Split the bits since the last gap into frames, skipping bits until
    /// one lines up (the host's acknowledge clock can leave a stray bit).
    fn decode_frames(&mut self) {
        let bits = std::mem::take(&mut self.bits);
        let len = match self.mode {
            Lk250Mode::Xt => 10,
            Lk250Mode::At => 11,
        };
        let mut rest = &bits[..];
        while rest.len() >= len {
            let frame = &rest[..len];
            let valid = match self.mode {
                Lk250Mode::Xt => frame[0],
                Lk250Mode::At => {
                    let ones = frame[1..10].iter().filter(|&&b| b).count();
                    !frame[0] && frame[10] && ones % 2 == 1
                }
            };
            if valid {
                let byte = (0..8).fold(0u8, |byte, i| byte | (frame[1 + i] as u8) << i);
                self.received.push_back(byte);
                rest = &rest[len..];
            } else {
                rest = &rest[1..];
            }
        }
    }

    fn start_send(&mut self, byte: u8) {
        let mut bits: Vec<bool> = (0..8).map(|i| byte >> i & 1 != 0).collect();
        bits.push(byte.count_ones() % 2 == 0);
        bits.push(true);
        self.system.ports.host_clock_low = true;
        self.system.ports.host_data_low = true;
        self.host = HostSend::Inhibit {
            ticks: HOST_INHIBIT,
            bits,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Key;

    fn run(hw: &mut LK250Hardware, ticks: usize) -> Vec<u8> {
        for _ in 0..ticks {
            hw.tick();
        }
        std::iter::from_fn(|| hw.recv()).collect()
    }

    fn type_a(hw: &mut LK250Hardware) -> Vec<u8> {
        let a = ScanCell::from_key(Key::Char('a')).unwrap();
        hw.press_key(a);
        let mut out = run(hw, 0x8000);
        hw.release_key(a);
        out.extend(run(hw, 0x8000));
        out
    }

    #[test]
    fn xt_power_up_and_key() {
        let mut hw = LK250Hardware::new(Lk250Mode::Xt);
        assert_eq!(run(&mut hw, 0x80000), vec![0xAA]);
        assert_eq!(type_a(&mut hw), vec![0x1E, 0x9E]);
    }

    #[test]
    fn at_power_up_and_key() {
        let mut hw = LK250Hardware::new(Lk250Mode::At);
        assert_eq!(run(&mut hw, 0x80000), vec![0xAA]);
        assert_eq!(type_a(&mut hw), vec![0x1C, 0xF0, 0x1C]);
    }

    #[test]
    fn at_commands() {
        let mut hw = LK250Hardware::new(Lk250Mode::At);
        run(&mut hw, 0x80000);
        hw.send(0xEE);
        assert_eq!(run(&mut hw, 0x20000), vec![0xEE]);
        hw.send(0xF2);
        let id = run(&mut hw, 0x20000);
        eprintln!("identify -> {id:02X?}");
        assert_eq!(id.first(), Some(&0xFA));
        let before = hw.leds();
        hw.send(0xED);
        hw.send(0x07);
        assert_eq!(run(&mut hw, 0x20000), vec![0xFA, 0xFA]);
        eprintln!("leds {before:02X} -> {:02X}", hw.leds());
        hw.send(0xFF);
        assert_eq!(run(&mut hw, 0x80000), vec![0xFA, 0xAA]);
    }
}
