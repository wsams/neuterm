//! PTY sessions, VT parsing, and a simple cell grid.
//!
//! See `specs/architecture.md` and `specs/performance.md`.

mod color;
mod grid;
mod pty;

pub use color::{Color, NamedColor};
pub use grid::{Cell, CellAttr, GridSnapshot, TerminalGrid};
pub use pty::{PtyEvent, PtyHandle, PtySession, SpawnOptions, WakeFn};

use std::sync::Arc;

use parking_lot::RwLock;
use uuid::Uuid;

/// Unique id for a terminal pane session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(pub Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared terminal state for a pane.
#[derive(Clone)]
pub struct TerminalState {
    pub id: SessionId,
    pub grid: Arc<RwLock<TerminalGrid>>,
    pub title: Arc<RwLock<String>>,
    pub process_name: Arc<RwLock<String>>,
}

impl TerminalState {
    pub fn new(cols: u16, rows: u16, scrollback: usize) -> Self {
        Self {
            id: SessionId::new(),
            grid: Arc::new(RwLock::new(TerminalGrid::new(cols, rows, scrollback))),
            title: Arc::new(RwLock::new(String::from("Shell"))),
            process_name: Arc::new(RwLock::new(String::from("shell"))),
        }
    }

    pub fn snapshot(&self) -> GridSnapshot {
        self.grid.read().snapshot()
    }

    pub fn display_title(&self) -> String {
        let title = self.title.read().clone();
        if !title.is_empty() {
            title
        } else {
            self.process_name.read().clone()
        }
    }
}
