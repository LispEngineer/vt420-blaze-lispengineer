use std::fs;
use std::io;

use i8051::Cpu;
use i8051::sfr::{SFR_P1, SFR_P2, SFR_P3};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Offset, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;
use tracing::warn;

use crate::host::screen::ratatui::{DisplayMode, DrawOptions, TextTerminal};
use crate::host::screen::text::TextScreen;
use crate::machine::generic::keyboard::KeyboardInput;
use crate::machine::generic::keyboard::lk201_input::Lk201Input;
use crate::machine::vt420::System as Vt420;

pub struct Screen<'a> {
    system: &'a Vt420,
    display_mode: DisplayMode,
}

impl<'a> Screen<'a> {
    pub fn new(system: &'a Vt420) -> Self {
        Self {
            system,
            display_mode: DisplayMode::Normal,
        }
    }

    pub fn display_mode(mut self, mode: DisplayMode) -> Self {
        self.display_mode = mode;
        self
    }
}

impl<'a> Widget for Screen<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.display_mode == DisplayMode::Normal {
            TextScreen::new(self.system).render(area, buf);
            return;
        }

        let vram = &self.system.memory.vram
            [self.system.memory.display_mapper.vram_offset_display() as usize..];
        let mapper = &self.system.memory.display_mapper;
        let vram_base = 0;

        let mut line = [0_u16; 256];
        let mut attr = [0_u8; 256];

        let Some(rows) = mapper.row_count(vram) else {
            return;
        };

        for mut row_idx in 0..=rows as u16 {
            let row = ((vram[vram_base + row_idx as usize * 2] as u16) >> 1) << 8;
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
            // Bit 2: double width
            // Bit 1: swap between screen 0 and screen 1 attributes
            let row_attrs = vram[vram_base + row_idx as usize * 2 + 1];
            let is_double_width = (row_attrs >> 2) & 3 != 0;
            // If true, force 132 characters per line
            let row_is_132 = vram[vram_base + row_idx as usize * 2] & 1 != 0;

            // Decode 12-bit character codes from packed 3-byte sequences
            let mut b = 0;
            let mut j = 0;

            // First segment: 72 chars, bytes 0-107
            for i in 0..108 {
                let char = vram[row as usize + i];
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
                let char = vram[row as usize + i];
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

            // Extract attributes
            for i in 1..133 {
                let bit = ((i % 4) * 2) as u8;
                attr[i - 1] = (vram[row as usize + 0xdd + (i / 4)] >> bit) & 0x3;
                let cell_attr = ((line[i - 1] & 0xf00) >> 8) as u8;
                attr[i - 1] |= cell_attr << 2;
            }

            // Render the line
            match self.display_mode {
                DisplayMode::Bytes => {
                    let row_header = format!("{:02X}|", row >> 8);
                    let mut col = 0;
                    for (i, b) in vram[row as usize..row as usize + 256].iter().enumerate() {
                        if col < area.width {
                            let hex_str = format!("{b:02X}");
                            for ch in hex_str.chars() {
                                if let Some(cell) =
                                    buf.cell_mut((area.left() + col, area.top() + row_idx))
                                {
                                    cell.set_symbol(&ch.to_string());
                                    let mut style = if i % 2 == 0 {
                                        Style::default()
                                    } else {
                                        Style::default().bold()
                                    };
                                    if i > 107 && i < 128 {
                                        style = style.fg(Color::Blue);
                                    }
                                    if i > 221 {
                                        style = style.fg(Color::Red);
                                    }
                                    cell.set_style(style);
                                }
                                col += 1;
                            }
                        }
                    }
                }
                DisplayMode::NibbleTriplet => {
                    let row_header = format!(
                        "{:02X}{:02X}|",
                        vram[vram_base + row_idx as usize * 2],
                        vram[vram_base + row_idx as usize * 2 + 1]
                    );
                    let mut col = 0;
                    for ch in row_header.chars() {
                        if col < area.width {
                            if let Some(cell) =
                                buf.cell_mut((area.left() + col, area.top() + row_idx))
                            {
                                cell.set_symbol(&ch.to_string());
                                cell.set_style(Style::default());
                            }
                            col += 1;
                        }
                    }
                    for (i, char_code) in line.iter().take(132).enumerate() {
                        let hex_str = format!("{char_code:03X}");
                        for ch in hex_str.chars() {
                            if col < area.width {
                                if let Some(cell) =
                                    buf.cell_mut((area.left() + col, area.top() + row_idx))
                                {
                                    cell.set_symbol(&ch.to_string());
                                    cell.set_style(if i % 2 == 0 {
                                        Style::default()
                                    } else {
                                        Style::default().bold()
                                    });
                                }
                                col += 1;
                            }
                        }
                    }
                }
                DisplayMode::Normal => {}
            }
        }
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

    fn draw(&self, cpu: &Cpu, options: &DrawOptions, f: &mut Frame) {
        let screen = Screen::new(self).display_mode(options.mode);
        f.render_widget(screen, f.area());
        let stage = Span::styled(
            format!(
                "{:b}/{:02X}",
                cpu.internal_ram[0x1f], cpu.internal_ram[0x7e]
            ),
            Style::default().fg(Color::LightBlue),
        );
        let stage = stage.into_right_aligned_line();
        f.render_widget(stage, f.area());

        if options.show_mapper {
            let mut mapper_line = Line::default();
            for i in 0..16 {
                let attr = self.memory.mapper.get(i);
                let style = Style::default().fg(Color::Indexed(attr));
                let text = if i == 6 || i == 9 || i == 10 || i == 11 || i == 12 {
                    Span::styled(
                        format!(
                            "{:02X}/{:02X} ",
                            self.memory.mapper.get(i),
                            self.memory.mapper.get2(i)
                        ),
                        style,
                    )
                } else {
                    Span::styled(format!("{:02X} ", self.memory.mapper.get(i)), style)
                };
                mapper_line.push_span(text);
            }
            mapper_line.push_span(format!(
                "{:02X} {:02X} {:02X}",
                cpu.sfr(SFR_P1, self),
                cpu.sfr(SFR_P2, self),
                cpu.sfr(SFR_P3, self)
            ));
            f.render_widget(mapper_line, f.area());
        }

        if options.show_vram {
            let vram = &self.memory.vram;
            for i in 0..16 {
                let mut vram_line = Line::default();
                for j in 0..32 {
                    let attr = vram[i * 32 + j];
                    let style = Style::default().fg(Color::Indexed(attr));
                    let text = Span::styled(format!("{attr:02X} "), style);
                    vram_line.push_span(text);
                }
                f.render_widget(
                    vram_line,
                    f.area().offset(Offset {
                        x: 0,
                        y: (f.area().height as i32 - 16) + i as i32,
                    }),
                );
            }
        }
    }
}
