use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::task::{Context, Poll};

use ssu::session::{SessionPartsUnsend, SessionRecvEndpoint, SessionSendEndpoint};
use tracing::{error, info};

use crate::machine::generic::display::{Display, text_lines};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Comm1(Vec<u8>),
    Comm2(Vec<u8>),
    Keyboard(Vec<u8>),
    Screenshot(PathBuf),
    Expect(String),
    Exit(i32),
    WaitInstructions(usize),
    WaitTicks(usize),
}

#[derive(Debug, PartialEq, Eq)]
enum Arg {
    Text(String),
    Number(i64),
}

fn bytes(text: &str) -> Vec<u8> {
    text.chars().map(|c| c as u8).collect()
}

fn parse_number(token: &str) -> Result<i64, String> {
    let (negative, digits) = match token.strip_prefix('-') {
        Some(digits) => (true, digits),
        None => (false, token),
    };
    let value = match digits.strip_prefix("0x") {
        Some(hex) => i64::from_str_radix(hex, 16),
        None => digits.parse(),
    }
    .map_err(|e| format!("bad number {token:?}: {e}"))?;
    Ok(if negative { -value } else { value })
}

fn parse_string(input: &str) -> Result<(String, &str), String> {
    let mut text = String::new();
    let mut chars = input.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Ok((text, &input[i + 1..])),
            '\\' => {
                let escaped = match chars.next().map(|(_, c)| c) {
                    Some('n') => '\n',
                    Some('r') => '\r',
                    Some('t') => '\t',
                    Some('e') => '\x1b',
                    Some('0') => '\0',
                    Some('\\') => '\\',
                    Some('"') => '"',
                    Some('x') => {
                        let hex: String = chars.by_ref().take(2).map(|(_, c)| c).collect();
                        let byte = u8::from_str_radix(&hex, 16)
                            .map_err(|_| format!("bad escape \\x{hex}"))?;
                        byte as char
                    }
                    Some(c) => return Err(format!("bad escape \\{c}")),
                    None => break,
                };
                text.push(escaped);
            }
            c => text.push(c),
        }
    }
    Err("unterminated string".into())
}

fn parse_arg(input: &str) -> Result<(Arg, &str), String> {
    if let Some(rest) = input.strip_prefix('"') {
        let (text, rest) = parse_string(rest)?;
        return Ok((Arg::Text(text), rest));
    }
    let end = input
        .find(|c: char| c.is_whitespace() || c == '#')
        .unwrap_or(input.len());
    if end == 0 {
        return Err("missing argument".into());
    }
    Ok((Arg::Number(parse_number(&input[..end])?), &input[end..]))
}

fn parse_line(line: &str) -> Result<Option<(u32, Action)>, String> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return Ok(None);
    }
    let (pc, rest) = line
        .split_once(':')
        .ok_or("expected `PC: command argument`")?;
    let pc = pc.trim();
    let pc = u32::from_str_radix(pc.strip_prefix("0x").unwrap_or(pc), 16)
        .map_err(|e| format!("bad PC {pc:?}: {e}"))?;
    let rest = rest.trim_start();
    let (command, rest) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    let (arg, rest) = parse_arg(rest.trim_start())?;
    let rest = rest.trim_start();
    if !rest.is_empty() && !rest.starts_with('#') {
        return Err(format!("unexpected {rest:?} after argument"));
    }
    let action = match (command, arg) {
        ("comm1", Arg::Text(text)) => Action::Comm1(bytes(&text)),
        ("comm2", Arg::Text(text)) => Action::Comm2(bytes(&text)),
        ("keyboard", Arg::Text(text)) => Action::Keyboard(bytes(&text)),
        ("screenshot", Arg::Text(text)) => Action::Screenshot(text.into()),
        ("expect", Arg::Text(text)) => Action::Expect(text),
        ("exit", Arg::Number(n)) => {
            Action::Exit(i32::try_from(n).map_err(|_| format!("bad exit code {n}"))?)
        }
        ("wait_instructions", Arg::Number(n)) => Action::WaitInstructions(
            usize::try_from(n).map_err(|_| format!("bad instruction count {n}"))?,
        ),
        ("wait_ticks", Arg::Number(n)) => {
            Action::WaitTicks(usize::try_from(n).map_err(|_| format!("bad tick count {n}"))?)
        }
        ("comm1" | "comm2" | "keyboard" | "screenshot" | "expect", _) => {
            return Err(format!("{command} needs a quoted string"));
        }
        ("exit" | "wait_instructions" | "wait_ticks", _) => {
            return Err(format!("{command} needs a number"));
        }
        _ => return Err(format!("unknown command {command:?}")),
    };
    Ok(Some((pc, action)))
}

pub trait ScriptHost: Display {
    fn script_keyboard(&mut self, bytes: &[u8]);
    fn script_instructions(&self) -> usize;
    fn script_ticks(&self) -> usize;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Wait {
    Instructions(usize),
    Ticks(usize),
}

type CommQueue = Rc<RefCell<VecDeque<u8>>>;

struct ScriptRecv {
    queue: CommQueue,
    inner: Option<Box<dyn SessionRecvEndpoint>>,
}

impl fmt::Debug for ScriptRecv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ScriptRecv(len={})", self.queue.borrow().len())
    }
}

impl SessionRecvEndpoint for ScriptRecv {
    fn poll_recv(&mut self, ctx: &mut Context<'_>) -> Poll<io::Result<u8>> {
        if let Some(byte) = self.queue.borrow_mut().pop_front() {
            return Poll::Ready(Ok(byte));
        }
        match &mut self.inner {
            Some(inner) => inner.poll_recv(ctx),
            None => Poll::Pending,
        }
    }
}

#[derive(Debug)]
struct ScriptSend {
    inner: Option<Box<dyn SessionSendEndpoint>>,
}

impl SessionSendEndpoint for ScriptSend {
    fn poll_send(&mut self, ctx: &mut Context<'_>, b: u8) -> Poll<io::Result<()>> {
        match &mut self.inner {
            Some(inner) => inner.poll_send(ctx, b),
            None => Poll::Ready(Ok(())),
        }
    }
}

#[derive(Default)]
pub struct Script {
    steps: Vec<(u32, Action)>,
    next: usize,
    wait: Option<Wait>,
    exit: Option<i32>,
    comm: [CommQueue; 2],
}

impl Script {
    pub fn comm_session(
        &self,
        channel: usize,
        inner: Option<SessionPartsUnsend>,
    ) -> SessionPartsUnsend {
        let (send, recv) = match inner {
            Some(parts) => (Some(parts.send), Some(parts.recv)),
            None => (None, None),
        };
        SessionPartsUnsend::new(
            ScriptSend { inner: send },
            ScriptRecv {
                queue: self.comm[channel].clone(),
                inner: recv,
            },
        )
    }

    pub fn parse(script: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut steps = vec![];
        for (n, line) in script.lines().enumerate() {
            if let Some(step) = parse_line(line).map_err(|e| format!("line {}: {e}", n + 1))? {
                steps.push(step);
            }
        }
        Ok(Self {
            steps,
            ..Default::default()
        })
    }

    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::parse(&std::fs::read_to_string(path)?)
    }

    pub fn is_done(&self) -> bool {
        self.exit.is_some() || (self.next == self.steps.len() && self.wait.is_none())
    }

    pub fn exit_code(&self) -> Option<i32> {
        self.exit
    }

    pub fn run<H: ScriptHost>(&mut self, pc: u32, host: &mut H) {
        while self.exit.is_none() {
            match self.wait {
                Some(Wait::Instructions(until)) if host.script_instructions() < until => return,
                Some(Wait::Ticks(until)) if host.script_ticks() < until => return,
                _ => self.wait = None,
            }
            let Some((step_pc, action)) = self.steps.get(self.next).cloned() else {
                return;
            };
            if step_pc != pc {
                return;
            }
            self.next += 1;
            info!("Script at {pc:05X}: {action:?}");
            self.apply(pc, action, host);
        }
    }

    fn apply<H: ScriptHost>(&mut self, pc: u32, action: Action, host: &mut H) {
        match action {
            Action::Comm1(bytes) => self.comm[0].borrow_mut().extend(bytes),
            Action::Comm2(bytes) => self.comm[1].borrow_mut().extend(bytes),
            Action::Keyboard(bytes) => host.script_keyboard(&bytes),
            Action::Screenshot(path) => {
                #[cfg(feature = "vram-dump")]
                if let Err(e) = crate::machine::generic::display::save_png(host, &path) {
                    error!("Script at {pc:05X}: screenshot {path:?} failed: {e}");
                    self.exit = Some(1);
                }
                #[cfg(not(feature = "vram-dump"))]
                {
                    error!("Script at {pc:05X}: screenshot {path:?} needs the vram-dump feature");
                    self.exit = Some(1);
                }
            }
            Action::Expect(text) => {
                let screen = text_lines(host).join("\n");
                if !screen.contains(&text) {
                    error!("Script at {pc:05X}: expected {text:?} on screen:\n{screen}");
                    self.exit = Some(1);
                }
            }
            Action::Exit(code) => self.exit = Some(code),
            Action::WaitInstructions(n) => {
                self.wait = Some(Wait::Instructions(host.script_instructions() + n))
            }
            Action::WaitTicks(n) => self.wait = Some(Wait::Ticks(host.script_ticks() + n)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::generic::display::{TextAttr, TextLine};
    use ssu::session::Session;
    use ssu::session::SyncSession;
    use ssu::session::loopback::LoopbackConfig;

    #[derive(Default)]
    struct Host {
        keyboard: Vec<u8>,
        instructions: usize,
        ticks: usize,
        screen: &'static str,
    }

    impl Display for Host {
        fn render_framebuffer(&self, _frame: &mut [u8]) {}

        fn render_textbuffer(
            &self,
            _line: &mut dyn FnMut(usize, TextLine),
            cell: &mut dyn FnMut(usize, usize, char, TextAttr),
        ) {
            for (column, ch) in self.screen.chars().enumerate() {
                cell(0, column, ch, TextAttr::NONE);
            }
        }
    }

    impl ScriptHost for Host {
        fn script_keyboard(&mut self, bytes: &[u8]) {
            self.keyboard.extend(bytes);
        }
        fn script_instructions(&self) -> usize {
            self.instructions
        }
        fn script_ticks(&self) -> usize {
            self.ticks
        }
    }

    #[test]
    fn parses_lines() {
        let script = Script::parse(
            r#"
            # comment
            0x10000: comm1 "ab\x80\e[2J\"\\\r\n"
            10000: keyboard "\xF0\x1C"  # trailing comment
            0x12345: screenshot "abc.png"
            0x12345: expect "Selftest OK"
            0x12345: wait_instructions 0x100
            0x12345: wait_ticks 60
            0x12345: exit -1
            "#,
        )
        .unwrap();
        assert_eq!(
            script.steps,
            vec![
                (0x10000, Action::Comm1(b"ab\x80\x1b[2J\"\\\r\n".to_vec())),
                (0x10000, Action::Keyboard(vec![0xF0, 0x1C])),
                (0x12345, Action::Screenshot("abc.png".into())),
                (0x12345, Action::Expect("Selftest OK".into())),
                (0x12345, Action::WaitInstructions(0x100)),
                (0x12345, Action::WaitTicks(60)),
                (0x12345, Action::Exit(-1)),
            ]
        );
    }

    #[test]
    fn rejects_bad_lines() {
        for line in [
            "0x1: comm3 \"x\"",
            "0x1 comm1 \"x\"",
            "zz: comm1 \"x\"",
            "0x1: comm1 5",
            "0x1: exit \"x\"",
            "0x1: comm1 \"x",
            "0x1: comm1 \"\\q\"",
            "0x1: comm1 \"x\" y",
            "0x1: wait_ticks -1",
            "0x1: exit",
        ] {
            assert!(Script::parse(line).is_err(), "{line}");
        }
        let err = Script::parse("# ok\n0x1: nope 1").err().unwrap();
        assert!(err.to_string().starts_with("line 2:"), "{err}");
    }

    #[test]
    fn runs_steps_in_order() {
        let mut script =
            Script::parse("0x1: wait_ticks 2\n0x1: comm2 \"x\"\n0x2: expect \"OK\"\n0x2: exit 3")
                .unwrap();
        let mut comm = SyncSession::new(script.comm_session(1, None));
        let mut host = Host {
            screen: "OK",
            ..Default::default()
        };
        script.run(1, &mut host);
        host.ticks = 2;
        script.run(2, &mut host);
        assert_eq!(comm.try_recv().unwrap(), None);
        script.run(1, &mut host);
        assert_eq!(comm.try_recv().unwrap(), Some(b'x'));
        assert_eq!(comm.try_recv().unwrap(), None);
        assert!(comm.try_send(b'y').unwrap().is_ok());
        script.run(2, &mut host);
        assert_eq!(script.exit_code(), Some(3));
    }

    #[test]
    fn comm_session_wraps_inner() {
        let script = Script::parse("0x1: comm1 \"ab\"").unwrap();
        let mut script = script;
        let inner = LoopbackConfig::default().boot().unwrap().into();
        let mut comm = SyncSession::new(script.comm_session(0, Some(inner)));
        script.run(1, &mut Host::default());
        comm.try_send(b'z').unwrap().unwrap();
        assert_eq!(comm.try_recv().unwrap(), Some(b'a'));
        assert_eq!(comm.try_recv().unwrap(), Some(b'b'));
        assert_eq!(comm.try_recv().unwrap(), Some(b'z'));
        assert_eq!(comm.try_recv().unwrap(), None);
    }
}
