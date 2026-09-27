use std::io;
use std::time::{Duration, Instant};

use i8051::Cpu;
use i8051_debug_tui::Debugger;
use ratatui::Frame;
use ratatui::crossterm;
use ratatui::layout::Offset;
use ratatui::prelude::CrosstermBackend;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use tracing::warn;

use crate::host::keyboard::crossterm::{CrosstermKeyboard, KeyboardCommand};
use crate::host::screen::text::TextScreen;
use crate::host::screen::text_terminal::{
    Anchor, DisplayMode, DrawOptions, TextStyle, TextTerminal,
};

fn style(style: TextStyle) -> Style {
    let mut out = Style::default();
    if style.bold {
        out = out.bold();
    }
    if let Some(color) = style.color {
        out = out.fg(Color::Indexed(color));
    }
    out
}

fn draw<S: TextTerminal>(system: &S, cpu: &Cpu, options: &DrawOptions, f: &mut Frame) {
    let area = f.area();

    if options.mode == DisplayMode::Normal {
        f.render_widget(TextScreen::new(system), area);
    } else {
        let buf = f.buffer_mut();
        system.debug_cells(options.mode, &mut |row, col, ch, cell_style| {
            if row >= area.height as usize || col >= area.width as usize {
                return;
            }
            if let Some(cell) = buf.cell_mut((area.left() + col as u16, area.top() + row as u16)) {
                cell.set_char(ch);
                cell.set_style(style(cell_style));
            }
        });
    }

    for overlay in system.overlays(cpu, options) {
        let line = Line::from(
            overlay
                .spans
                .into_iter()
                .map(|(text, span_style)| Span::styled(text, style(span_style)))
                .collect::<Vec<_>>(),
        );
        match overlay.anchor {
            Anchor::Top => f.render_widget(line, area),
            Anchor::TopRight => f.render_widget(line.right_aligned(), area),
            Anchor::Bottom(lines) => {
                let y = (area.height as usize).saturating_sub(lines) as i32;
                f.render_widget(line, area.offset(Offset { x: 0, y }));
            }
        }
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

            terminal.draw(|f| draw(&system, &cpu, &options, f))?;
        }

        #[cfg(all(feature = "pc-trace", not(target_arch = "wasm32")))]
        system.flush_pc_trace_if_due();
    }
}
