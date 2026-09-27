use std::fs;
use std::io;

use i8051::Cpu;
use i8051::sfr::{SFR_P1, SFR_P2, SFR_P3};
use tracing::warn;

use crate::host::screen::text_terminal::{
    Anchor, DisplayMode, DrawOptions, Overlay, TextStyle, TextTerminal,
};
use crate::machine::generic::keyboard::KeyboardInput;
use crate::machine::generic::keyboard::lk201_input::Lk201Input;
use crate::machine::vt420::System as Vt420;

const BLUE: u8 = 4;
const RED: u8 = 1;
const LIGHT_BLUE: u8 = 12;

fn alternate(i: usize) -> TextStyle {
    TextStyle {
        bold: i % 2 != 0,
        color: None,
    }
}

impl TextTerminal for Vt420 {
    fn keyboard_input(&self) -> Box<dyn KeyboardInput> {
        Box::new(Lk201Input::new(self.keyboard.sender()))
    }

    fn instruction_count(&self) -> usize {
        self.instruction_count
    }

    fn redraw_interval(&self) -> usize {
        0x1000
    }

    fn check_step(&self, pc: u32, new_pc: u32) {
        if new_pc & 0xffff == 0 {
            warn!("CPU reset detected at PC = 0x{:04X}", pc);
        }
        if (0xbb..0x110).contains(&new_pc) {
            warn!(
                "CPU weird step ({:02X}) detected at PC = 0x{:04X}",
                new_pc, pc
            );
        }
    }

    fn dump_vram(&self) -> io::Result<()> {
        fs::write("/tmp/vram.bin", &self.memory.vram[0..])
    }

    #[cfg(all(feature = "pc-trace", not(target_arch = "wasm32")))]
    fn flush_pc_trace_now(&mut self) -> io::Result<()> {
        if let Some(trace) = &mut self.pc_trace {
            trace.flush_now()?;
        }
        Ok(())
    }

    fn overlays(&self, cpu: &Cpu, options: &DrawOptions) -> Vec<Overlay> {
        let mut overlays = vec![Overlay {
            anchor: Anchor::TopRight,
            spans: vec![(
                format!(
                    "{:b}/{:02X}",
                    cpu.internal_ram[0x1f], cpu.internal_ram[0x7e]
                ),
                TextStyle::color(LIGHT_BLUE),
            )],
        }];

        if options.show_mapper {
            let mapper = &self.memory.mapper;
            let mut spans: Vec<(String, TextStyle)> = (0..16)
                .map(|i| {
                    let text = if matches!(i, 6 | 9 | 10 | 11 | 12) {
                        format!("{:02X}/{:02X} ", mapper.get(i), mapper.get2(i))
                    } else {
                        format!("{:02X} ", mapper.get(i))
                    };
                    (text, TextStyle::color(mapper.get(i)))
                })
                .collect();
            spans.push((
                format!(
                    "{:02X} {:02X} {:02X}",
                    cpu.sfr(SFR_P1, self),
                    cpu.sfr(SFR_P2, self),
                    cpu.sfr(SFR_P3, self)
                ),
                TextStyle::default(),
            ));
            overlays.push(Overlay {
                anchor: Anchor::Top,
                spans,
            });
        }

        if options.show_vram {
            let vram = &self.memory.vram;
            for i in 0..16 {
                let spans = (0..32)
                    .map(|j| {
                        let attr = vram[i * 32 + j];
                        (format!("{attr:02X} "), TextStyle::color(attr))
                    })
                    .collect();
                overlays.push(Overlay {
                    anchor: Anchor::Bottom(16 - i),
                    spans,
                });
            }
        }
        overlays
    }

    fn debug_cells(&self, mode: DisplayMode, cell: &mut dyn FnMut(usize, usize, char, TextStyle)) {
        let vram = &self.memory.vram[self.memory.display_mapper.vram_offset_display() as usize..];
        let mapper = &self.memory.display_mapper;
        let vram_base = 0;

        let mut line = [0_u16; 256];

        let Some(rows) = mapper.row_count(vram) else {
            return;
        };

        for mut row_idx in 0..=rows as usize {
            let row = ((vram[vram_base + row_idx * 2] as usize) >> 1) << 8;
            if row == 0 {
                continue;
            }
            // Handle smooth scrolling by chopping the top row
            if mapper.get(2) != 0 {
                if row_idx as u8 == mapper.get(0) {
                    continue;
                }
                if row_idx as u8 > mapper.get(0) {
                    row_idx -= 1;
                }
            }

            // Decode 12-bit character codes from packed 3-byte sequences
            let mut b = 0;
            let mut j = 0;

            // First segment: 72 chars, bytes 0-107
            for i in 0..108 {
                let char = vram[row + i];
                match i % 3 {
                    0 => b = char as u16,
                    1 => {
                        b |= ((char & 0xf) as u16) << 8;
                        line[j] = b;
                        j += 1;
                        b = ((char & 0xf0) as u16) >> 4;
                    }
                    _ => {
                        b |= (char as u16) << 4;
                        line[j] = b;
                        j += 1;
                    }
                }
            }
            // Second segment: bytes 128-220
            for i in 128..221 {
                let char = vram[row + i];
                let i = i + 1;
                match i % 3 {
                    0 => b = char as u16,
                    1 => {
                        b |= ((char & 0xf) as u16) << 8;
                        line[j] = b;
                        j += 1;
                        b = ((char & 0xf0) as u16) >> 4;
                    }
                    _ => {
                        b |= (char as u16) << 4;
                        line[j] = b;
                        j += 1;
                    }
                }
            }

            let mut col = 0;
            let mut put = |ch: char, style: TextStyle| {
                cell(row_idx, col, ch, style);
                col += 1;
            };
            match mode {
                DisplayMode::Bytes => {
                    for (i, b) in vram[row..row + 256].iter().enumerate() {
                        let mut style = alternate(i);
                        if i > 107 && i < 128 {
                            style.color = Some(BLUE);
                        }
                        if i > 221 {
                            style.color = Some(RED);
                        }
                        for ch in format!("{b:02X}").chars() {
                            put(ch, style);
                        }
                    }
                }
                DisplayMode::NibbleTriplet => {
                    let row_header = format!(
                        "{:02X}{:02X}|",
                        vram[vram_base + row_idx * 2],
                        vram[vram_base + row_idx * 2 + 1]
                    );
                    for ch in row_header.chars() {
                        put(ch, TextStyle::default());
                    }
                    for (i, char_code) in line.iter().take(132).enumerate() {
                        for ch in format!("{char_code:03X}").chars() {
                            put(ch, alternate(i));
                        }
                    }
                }
                DisplayMode::Normal => {}
            }
        }
    }
}
