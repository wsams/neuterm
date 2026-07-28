//! PTY spawn and VT byte pump.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use anyhow::{anyhow, Context, Result};
use parking_lot::RwLock;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use tracing::{debug, error, warn};
use vte::{Params, Parser, Perform};

use crate::grid::TerminalGrid;
use crate::TerminalState;

pub type WakeFn = Arc<dyn Fn() + Send + Sync + 'static>;

pub struct SpawnOptions {
    pub cols: u16,
    pub rows: u16,
    pub scrollback: usize,
    pub shell: Option<String>,
    pub cwd: Option<PathBuf>,
    pub term_program: String,
    /// Called from the PTY reader thread whenever output arrives (wake the UI).
    pub on_output: Option<WakeFn>,
}

pub enum PtyEvent {
    Output,
    Bell,
    Title(String),
    Exit,
}

pub struct PtyHandle {
    pub state: TerminalState,
    writer: Arc<Mutex<Option<Box<dyn Write + Send>>>>,
    events: Receiver<PtyEvent>,
    _reader: JoinHandle<()>,
    _child_waiter: JoinHandle<()>,
}

impl PtyHandle {
    pub fn write_all(&self, data: &[u8]) -> Result<()> {
        let mut guard = self
            .writer
            .lock()
            .map_err(|_| anyhow!("pty writer lock poisoned"))?;
        if let Some(w) = guard.as_mut() {
            w.write_all(data)?;
            w.flush()?;
        }
        Ok(())
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        self.state.grid.write().resize(cols, rows);
        // Master resize is done via stored master — we keep a channel for resize.
        // For MVP, grid resize is enough; PTY size updated through ResizeCmd below.
        let _ = cols;
        let _ = rows;
        Ok(())
    }

    pub fn poll_events(&self) -> Vec<PtyEvent> {
        let mut out = Vec::new();
        while let Ok(ev) = self.events.try_recv() {
            out.push(ev);
        }
        out
    }
}

/// Owns the master PTY for resize.
pub struct PtySession {
    handle: PtyHandle,
    master: Arc<RwLock<Box<dyn MasterPty + Send>>>,
}

impl PtySession {
    pub fn spawn(opts: SpawnOptions) -> Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: opts.rows,
                cols: opts.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("openpty")?;

        let shell = resolve_shell(opts.shell.as_deref())?;
        let mut cmd = CommandBuilder::new(&shell);
        // Interactive shell (not login): more reliable inside an emulator PTY.
        // Fish/bash/zsh all accept -i; login shells can be enabled later via config.
        #[cfg(unix)]
        {
            let name = shell
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if name.contains("fish") {
                // fish: just run as interactive; -i is default for tty
            } else if name.contains("zsh") {
                cmd.arg("-i");
            } else {
                // bash / sh
                cmd.arg("-i");
            }
        }
        cmd.env("TERM", &opts.term_program);
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TERM_PROGRAM", "neuterm");
        if let Some(cwd) = &opts.cwd {
            cmd.cwd(cwd);
        }

        let child = pair.slave.spawn_command(cmd).context("spawn shell")?;
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader().context("clone pty reader")?;
        let writer = pair.master.take_writer().context("take pty writer")?;
        let writer = Arc::new(Mutex::new(Some(writer)));
        let master = Arc::new(RwLock::new(pair.master));

        let state = TerminalState::new(opts.cols, opts.rows, opts.scrollback);
        *state.process_name.write() = shell
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "shell".into());

        let (tx, rx) = mpsc::channel();
        let grid = Arc::clone(&state.grid);
        let title_arc = Arc::clone(&state.title);
        let tx_reader = tx.clone();
        let on_output = opts.on_output.clone();
        let writer_for_reader = Arc::clone(&writer);

        let reader_thread = thread::Builder::new()
            .name("neuterm-pty-read".into())
            .spawn(move || {
                let mut parser = Parser::new();
                let mut performer = Performer {
                    grid,
                    title: title_arc,
                    writer: writer_for_reader,
                    events: tx_reader.clone(),
                    bell: false,
                };
                let mut buf = [0u8; 8192];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => {
                            let _ = tx_reader.send(PtyEvent::Exit);
                            if let Some(wake) = &on_output {
                                wake();
                            }
                            break;
                        }
                        Ok(n) => {
                            performer.bell = false;
                            for &byte in &buf[..n] {
                                parser.advance(&mut performer, byte);
                            }
                            let _ = tx_reader.send(PtyEvent::Output);
                            if performer.bell {
                                let _ = tx_reader.send(PtyEvent::Bell);
                            }
                            if let Some(wake) = &on_output {
                                wake();
                            }
                        }
                        Err(err) => {
                            error!("pty read error: {err}");
                            let _ = tx_reader.send(PtyEvent::Exit);
                            if let Some(wake) = &on_output {
                                wake();
                            }
                            break;
                        }
                    }
                }
            })
            .context("spawn reader thread")?;

        let mut child = child;
        let waiter = thread::Builder::new()
            .name("neuterm-pty-wait".into())
            .spawn(move || {
                match child.wait() {
                    Ok(status) => debug!("child exited: {status:?}"),
                    Err(err) => warn!("child wait error: {err}"),
                }
                let _ = tx.send(PtyEvent::Exit);
            })
            .context("spawn waiter thread")?;

        let handle = PtyHandle {
            state,
            writer,
            events: rx,
            _reader: reader_thread,
            _child_waiter: waiter,
        };

        Ok(Self { handle, master })
    }

    pub fn handle(&self) -> &PtyHandle {
        &self.handle
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        self.handle.state.grid.write().resize(cols, rows);
        self.master.write().resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        Ok(())
    }

    pub fn write_all(&self, data: &[u8]) -> Result<()> {
        self.handle.write_all(data)
    }

    pub fn poll_events(&self) -> Vec<PtyEvent> {
        self.handle.poll_events()
    }

    pub fn state(&self) -> &TerminalState {
        &self.handle.state
    }
}

fn resolve_shell(configured: Option<&str>) -> Result<PathBuf> {
    if let Some(s) = configured {
        return Ok(PathBuf::from(s));
    }
    if let Ok(shell) = std::env::var("SHELL") {
        return Ok(PathBuf::from(shell));
    }
    #[cfg(windows)]
    {
        if let Ok(comspec) = std::env::var("COMSPEC") {
            return Ok(PathBuf::from(comspec));
        }
        return Ok(PathBuf::from("powershell.exe"));
    }
    #[cfg(not(windows))]
    {
        for candidate in ["/bin/bash", "/bin/sh"] {
            let p = PathBuf::from(candidate);
            if p.exists() {
                return Ok(p);
            }
        }
        Err(anyhow!("could not find a shell"))
    }
}

struct Performer {
    grid: Arc<RwLock<TerminalGrid>>,
    title: Arc<RwLock<String>>,
    writer: Arc<Mutex<Option<Box<dyn Write + Send>>>>,
    events: Sender<PtyEvent>,
    bell: bool,
}

impl Performer {
    fn reply(&self, bytes: &[u8]) {
        if let Ok(mut guard) = self.writer.lock() {
            if let Some(w) = guard.as_mut() {
                let _ = w.write_all(bytes);
                let _ = w.flush();
            }
        }
    }
}

impl Perform for Performer {
    fn print(&mut self, c: char) {
        self.grid.write().put_char(c);
    }

    fn execute(&mut self, byte: u8) {
        let mut g = self.grid.write();
        match byte {
            b'\n' => g.line_feed(),
            b'\r' => g.carriage_return(),
            b'\t' => g.put_char('\t'),
            0x08 => g.backspace(),
            0x07 => self.bell = true,
            _ => {}
        }
    }

    fn hook(&mut self, _params: &Params, _intermediates: &[u8], _ignore: bool, _action: char) {}
    fn put(&mut self, _byte: u8) {}
    fn unhook(&mut self) {}

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if params.is_empty() {
            return;
        }
        let Ok(code) = std::str::from_utf8(params[0]) else {
            return;
        };
        // OSC 0 / 2 — set title
        if (code == "0" || code == "2") && params.len() >= 2 {
            if let Ok(title) = std::str::from_utf8(params[1]) {
                *self.title.write() = title.to_string();
                self.grid.write().set_title(title);
                let _ = self.events.send(PtyEvent::Title(title.to_string()));
            }
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, action: char) {
        let nums: Vec<i64> = params.iter().map(|p| p[0] as i64).collect();
        let first = |default: u16| -> u16 {
            nums.first()
                .copied()
                .filter(|&n| n > 0)
                .unwrap_or(default as i64) as u16
        };

        // Device Attribute / Status replies (fish waits on these).
        if action == 'c' && intermediates.is_empty() {
            // Primary DA — VT220-ish with color.
            self.reply(b"\x1b[?62;1;2;6;22c");
            return;
        }
        if action == 'c' && intermediates == [b'>'] {
            // Secondary DA
            self.reply(b"\x1b[>0;10;1c");
            return;
        }
        if action == 'n' && intermediates.is_empty() {
            let mode = nums.first().copied().unwrap_or(0);
            if mode == 5 {
                self.reply(b"\x1b[0n");
                return;
            }
            if mode == 6 {
                let snap = self.grid.read().snapshot();
                let reply = format!("\x1b[{};{}R", snap.cursor_row + 1, snap.cursor_col + 1);
                self.reply(reply.as_bytes());
                return;
            }
        }

        let mut g = self.grid.write();
        match action {
            'A' => g.cursor_up(first(1)),
            'B' => g.cursor_down(first(1)),
            'C' => g.cursor_forward(first(1)),
            'D' => g.cursor_back(first(1)),
            'H' | 'f' => {
                let row = nums.first().copied().unwrap_or(1).max(1) as u16;
                let col = nums.get(1).copied().unwrap_or(1).max(1) as u16;
                g.cursor_goto(row, col);
            }
            'J' => g.erase_in_display(nums.first().copied().unwrap_or(0) as u16),
            'K' => g.erase_in_line(nums.first().copied().unwrap_or(0) as u16),
            'm' => g.sgr(&nums),
            's' => g.save_cursor(),
            'u' => g.restore_cursor(),
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {}
}
