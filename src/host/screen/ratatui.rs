use std::io;
use std::time::{Duration, Instant};

use i8051::Cpu;
use i8051_debug_tui::Debugger;
use ratatui::Frame;
use ratatui::crossterm;
use ratatui::prelude::CrosstermBackend;
use tracing::warn;

use crate::host::keyboard::crossterm::{CrosstermKeyboard, KeyboardCommand};
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

pub trait TextTerminal: TerminalSystem + Display {
    fn keyboard_input(&self) -> Box<dyn KeyboardInput>;
    fn instruction_count(&self) -> usize;
    fn redraw_interval(&self) -> usize;
    fn draw(&self, cpu: &Cpu, options: &DrawOptions, frame: &mut Frame);

    fn check_step(&self, _pc: u32, _new_pc: u32) {}

    fn dump_vram(&self) -> io::Result<()> {
        Ok(())
    }

    #[cfg(all(feature = "pc-trace", not(target_arch = "wasm32")))]
    fn flush_pc_trace_now(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn run<S: TextTerminal>(
    system: S,
    cpu: Cpu,
    debugger: Option<Debugger>,
    show_mapper: bool,
    show_vram: bool,
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(io::stdout(), crossterm::terminal::EnterAlternateScreen,)?;
    crossterm::execute!(
        io::stdout(),
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
    )?;

    let options = DrawOptions {
        mode: DisplayMode::Normal,
        show_mapper,
        show_vram,
    };
    let result = run_inner(system, cpu, debugger, options);

    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen,)?;
    result
}

fn run_inner<S: TextTerminal>(
    mut system: S,
    mut cpu: Cpu,
    _debugger: Option<Debugger>,
    mut options: DrawOptions,
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    let mut running = true;
    let mut keyboard = CrosstermKeyboard::default();
    let mut input = system.keyboard_input();
    let mut terminal = ratatui::Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let redraw_interval = system.redraw_interval();
    loop {
        if running {
            let pc = cpu.pc_ext(&system);
            system.step(&mut cpu);
            system.check_step(pc, cpu.pc_ext(&system));
        }

        if system.instruction_count() % redraw_interval == 0 || !running {
            while crossterm::event::poll(Duration::from_millis(0))? {
                let start = Instant::now();
                let event = crossterm::event::read()?;
                if start.elapsed() > Duration::from_millis(100) {
                    warn!("Event read took too long: {:?}", start.elapsed());
                }
                match keyboard.update_keyboard(&event, &mut *input) {
                    Some(KeyboardCommand::ToggleRun) => {
                        running = !running;
                    }
                    Some(KeyboardCommand::ToggleHexMode) => {
                        options.mode = match options.mode {
                            DisplayMode::Normal => DisplayMode::NibbleTriplet,
                            DisplayMode::NibbleTriplet => DisplayMode::Bytes,
                            DisplayMode::Bytes => DisplayMode::Normal,
                        };
                    }
                    Some(KeyboardCommand::DumpVRAM) => {
                        system.dump_vram()?;
                    }
                    #[cfg(all(feature = "pc-trace", not(target_arch = "wasm32")))]
                    Some(KeyboardCommand::FlushPCTrace) => {
                        system.flush_pc_trace_now()?;
                    }
                    Some(KeyboardCommand::Quit) => {
                        return Ok(system.instruction_count());
                    }
                    None => {}
                }
            }

            terminal.draw(|f| system.draw(&cpu, &options, f))?;
        }

        #[cfg(all(feature = "pc-trace", not(target_arch = "wasm32")))]
        system.flush_pc_trace_if_due();
    }
}
