//! Tabs, panes, and session multiplexing.
//!
//! See `specs/tabs.md` and `specs/panes.md`.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use neuterm_config::{Config, ScrollbackLines};
use neuterm_term::{PtyEvent, PtySession, SpawnOptions, WakeFn};
use tracing::warn;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PaneId(pub Uuid);

impl TabId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TabId {
    fn default() -> Self {
        Self::new()
    }
}

impl PaneId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for PaneId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    /// Side-by-side (vertical divider).
    Horizontal,
    /// Stacked (horizontal divider).
    Vertical,
}

#[derive(Debug, Clone)]
pub enum PaneNode {
    Leaf(PaneId),
    Split {
        direction: SplitDirection,
        ratio: f32,
        first: Box<PaneNode>,
        second: Box<PaneNode>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneKind {
    Terminal,
    AiChat,
}

pub struct Pane {
    pub id: PaneId,
    pub kind: PaneKind,
    pub session: Option<PtySession>,
}

pub struct Tab {
    pub id: TabId,
    pub title_override: Option<String>,
    pub tree: PaneNode,
    pub active_pane: PaneId,
}

impl Tab {
    pub fn title(&self, panes: &HashMap<PaneId, Pane>) -> String {
        if let Some(t) = &self.title_override {
            if !t.is_empty() {
                return t.clone();
            }
        }
        if let Some(pane) = panes.get(&self.active_pane) {
            if let Some(session) = &pane.session {
                return session.state().display_title();
            }
            if pane.kind == PaneKind::AiChat {
                return "AI Chat".into();
            }
        }
        "Shell".into()
    }
}

pub struct Mux {
    pub tabs: Vec<Tab>,
    pub active_tab: usize,
    pub panes: HashMap<PaneId, Pane>,
    cols: u16,
    rows: u16,
    scrollback: usize,
    shell: Option<String>,
    cwd: Option<PathBuf>,
    term_program: String,
    on_output: Option<WakeFn>,
}

impl Mux {
    pub fn new(config: &Config) -> Self {
        let scrollback = match config.scrollback.lines {
            ScrollbackLines::Limited(n) => n,
            // Practical soft cap for "unlimited" (see specs/performance.md).
            ScrollbackLines::Unlimited => 10_000_000,
        };
        Self {
            tabs: Vec::new(),
            active_tab: 0,
            panes: HashMap::new(),
            cols: config.window.initial_cols,
            rows: config.window.initial_rows,
            scrollback,
            shell: config.term.shell.clone(),
            cwd: config.term.cwd.as_ref().map(PathBuf::from),
            term_program: config.term.program.clone(),
            on_output: None,
        }
    }

    /// Install a wake callback invoked from PTY reader threads on output.
    pub fn set_wake(&mut self, wake: WakeFn) {
        self.on_output = Some(wake);
    }

    pub fn bootstrap(&mut self) -> Result<()> {
        self.new_tab()?;
        Ok(())
    }

    fn spawn_session(&self) -> Result<PtySession> {
        PtySession::spawn(SpawnOptions {
            cols: self.cols.max(1),
            rows: self.rows.max(1),
            scrollback: self.scrollback,
            shell: self.shell.clone(),
            cwd: self.cwd.clone(),
            term_program: self.term_program.clone(),
            on_output: self.on_output.clone(),
        })
    }

    pub fn new_tab(&mut self) -> Result<TabId> {
        let session = self.spawn_session()?;
        let pane_id = PaneId::new();
        let tab_id = TabId::new();
        self.panes.insert(
            pane_id,
            Pane {
                id: pane_id,
                kind: PaneKind::Terminal,
                session: Some(session),
            },
        );
        self.tabs.push(Tab {
            id: tab_id,
            title_override: None,
            tree: PaneNode::Leaf(pane_id),
            active_pane: pane_id,
        });
        self.active_tab = self.tabs.len() - 1;
        Ok(tab_id)
    }

    pub fn close_active_tab(&mut self) -> bool {
        if self.tabs.is_empty() {
            return false;
        }
        let tab = self.tabs.remove(self.active_tab);
        self.drop_tree_panes(&tab.tree);
        if self.tabs.is_empty() {
            return false;
        }
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        }
        true
    }

    fn drop_tree_panes(&mut self, node: &PaneNode) {
        match node {
            PaneNode::Leaf(id) => {
                self.panes.remove(id);
            }
            PaneNode::Split { first, second, .. } => {
                self.drop_tree_panes(first);
                self.drop_tree_panes(second);
            }
        }
    }

    pub fn activate_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active_tab = index;
        }
    }

    pub fn tab_next(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        self.active_tab = (self.active_tab + 1) % self.tabs.len();
    }

    pub fn tab_prev(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        if self.active_tab == 0 {
            self.active_tab = self.tabs.len() - 1;
        } else {
            self.active_tab -= 1;
        }
    }

    pub fn tab_move_left(&mut self) {
        if self.active_tab == 0 || self.tabs.len() < 2 {
            return;
        }
        self.tabs.swap(self.active_tab, self.active_tab - 1);
        self.active_tab -= 1;
    }

    pub fn tab_move_right(&mut self) {
        if self.active_tab + 1 >= self.tabs.len() {
            return;
        }
        self.tabs.swap(self.active_tab, self.active_tab + 1);
        self.active_tab += 1;
    }

    pub fn set_tab_title(&mut self, title: Option<String>) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.title_override = title.filter(|s| !s.is_empty());
        }
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active_tab)
    }

    pub fn active_pane_mut(&mut self) -> Option<&mut Pane> {
        let pane_id = self.tabs.get(self.active_tab)?.active_pane;
        self.panes.get_mut(&pane_id)
    }

    pub fn split_active(&mut self, direction: SplitDirection) -> Result<()> {
        let session = self.spawn_session()?;
        let new_id = PaneId::new();
        self.panes.insert(
            new_id,
            Pane {
                id: new_id,
                kind: PaneKind::Terminal,
                session: Some(session),
            },
        );

        let tab = self
            .tabs
            .get_mut(self.active_tab)
            .ok_or_else(|| anyhow::anyhow!("no active tab"))?;
        let active = tab.active_pane;
        replace_leaf_with_split(&mut tab.tree, active, new_id, direction)?;
        tab.active_pane = new_id;
        Ok(())
    }

    pub fn focus_direction(&mut self, dir: FocusDir) {
        let Some(tab) = self.tabs.get(self.active_tab) else {
            return;
        };
        let leaves = collect_leaves(&tab.tree);
        if leaves.len() < 2 {
            return;
        }
        let Some(idx) = leaves.iter().position(|id| *id == tab.active_pane) else {
            return;
        };
        let next = match dir {
            FocusDir::Left | FocusDir::Up => {
                if idx == 0 {
                    leaves.len() - 1
                } else {
                    idx - 1
                }
            }
            FocusDir::Right | FocusDir::Down => (idx + 1) % leaves.len(),
        };
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.active_pane = leaves[next];
        }
    }

    pub fn open_ai_chat(&mut self, title: &str) -> Result<()> {
        let new_id = PaneId::new();
        self.panes.insert(
            new_id,
            Pane {
                id: new_id,
                kind: PaneKind::AiChat,
                session: None,
            },
        );
        let tab = self
            .tabs
            .get_mut(self.active_tab)
            .ok_or_else(|| anyhow::anyhow!("no active tab"))?;
        let active = tab.active_pane;
        replace_leaf_with_split(&mut tab.tree, active, new_id, SplitDirection::Horizontal)?;
        tab.active_pane = new_id;
        // Store title on override for visibility in tab bar when focused
        if tab.title_override.is_none() {
            let _ = title;
        }
        Ok(())
    }

    pub fn resize_all(&mut self, cols: u16, rows: u16) {
        self.cols = cols.max(1);
        self.rows = rows.max(1);
        for pane in self.panes.values_mut() {
            if let Some(session) = &pane.session {
                if let Err(err) = session.resize(self.cols, self.rows) {
                    warn!("resize failed: {err}");
                }
            }
        }
    }

    pub fn write_to_active(&mut self, data: &[u8]) -> Result<()> {
        if let Some(pane) = self.active_pane_mut() {
            if let Some(session) = &pane.session {
                session.write_all(data)?;
            }
        }
        Ok(())
    }

    pub fn pump_events(&mut self) -> PumpResult {
        let mut dirty = false;
        let mut bell = false;
        let mut exited = Vec::new();
        for (id, pane) in &self.panes {
            if let Some(session) = &pane.session {
                for ev in session.poll_events() {
                    match ev {
                        PtyEvent::Output | PtyEvent::Title(_) => dirty = true,
                        PtyEvent::Bell => {
                            dirty = true;
                            bell = true;
                        }
                        PtyEvent::Exit => {
                            exited.push(*id);
                            dirty = true;
                        }
                    }
                }
            }
        }
        PumpResult {
            dirty,
            bell,
            exited,
        }
    }
}

pub struct PumpResult {
    pub dirty: bool,
    pub bell: bool,
    pub exited: Vec<PaneId>,
}

#[derive(Debug, Clone, Copy)]
pub enum FocusDir {
    Left,
    Right,
    Up,
    Down,
}

fn collect_leaves(node: &PaneNode) -> Vec<PaneId> {
    match node {
        PaneNode::Leaf(id) => vec![*id],
        PaneNode::Split { first, second, .. } => {
            let mut v = collect_leaves(first);
            v.extend(collect_leaves(second));
            v
        }
    }
}

fn replace_leaf_with_split(
    node: &mut PaneNode,
    target: PaneId,
    new_leaf: PaneId,
    direction: SplitDirection,
) -> Result<()> {
    match node {
        PaneNode::Leaf(id) if *id == target => {
            *node = PaneNode::Split {
                direction,
                ratio: 0.5,
                first: Box::new(PaneNode::Leaf(target)),
                second: Box::new(PaneNode::Leaf(new_leaf)),
            };
            Ok(())
        }
        PaneNode::Leaf(_) => Err(anyhow::anyhow!("leaf not found")),
        PaneNode::Split { first, second, .. } => {
            if replace_leaf_with_split(first, target, new_leaf, direction).is_ok() {
                return Ok(());
            }
            replace_leaf_with_split(second, target, new_leaf, direction)
        }
    }
}
