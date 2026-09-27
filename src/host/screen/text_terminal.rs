use std::io;

use i8051::Cpu;

use crate::machine::TerminalSystem;
use crate::machine::generic::display::Display;
use crate::machine::generic::keyboard::KeyboardInput;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DisplayMode {
    Normal,
    NibbleTriplet,
    Bytes,
}

pub struct DrawOptions {
    pub mode: DisplayMode,
    pub show_mapper: bool,
    pub show_vram: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextStyle {
    pub bold: bool,
    pub color: Option<u8>,
}

impl TextStyle {
    pub fn color(color: u8) -> Self {
        Self {
            bold: false,
            color: Some(color),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    Top,
    TopRight,
    Bottom(usize),
}

pub struct Overlay {
    pub anchor: Anchor,
    pub spans: Vec<(String, TextStyle)>,
}

pub trait TextTerminal: TerminalSystem + Display {
    fn keyboard_input(&self) -> Box<dyn KeyboardInput>;
    fn instruction_count(&self) -> usize;
    fn redraw_interval(&self) -> usize;

    fn overlays(&self, _cpu: &Cpu, _options: &DrawOptions) -> Vec<Overlay> {
        vec![]
    }

    fn debug_cells(
        &self,
        _mode: DisplayMode,
        _cell: &mut dyn FnMut(usize, usize, char, TextStyle),
    ) {
    }

    fn check_step(&self, _pc: u32, _new_pc: u32) {}

    fn dump_vram(&self) -> io::Result<()> {
        Ok(())
    }

    #[cfg(all(feature = "pc-trace", not(target_arch = "wasm32")))]
    fn flush_pc_trace_now(&mut self) -> io::Result<()> {
        Ok(())
    }
}
