//! Window loop and application state.

use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use neuterm_ai::{ChatSession, OllamaClient};
use neuterm_config::Config;
use neuterm_mux::{FocusDir, Mux, PaneKind, SplitDirection};
use neuterm_triggers::TriggerEngine;
use softbuffer::{Context as SbContext, Surface};
use tracing::{error, info, warn};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use crate::input::{action_from_key, key_to_pty_bytes, Action};
use crate::render::{HitTarget, OverlayView, Renderer, Selection};

/// Wakes the event loop when PTY output arrives.
#[derive(Debug, Clone, Copy)]
pub enum AppEvent {
    PtyOutput,
}

pub fn run(config: Config, config_path: Option<PathBuf>) -> Result<()> {
    let event_loop = EventLoop::<AppEvent>::with_user_event()
        .build()
        .context("create event loop")?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let proxy = event_loop.create_proxy();
    let mut app = NeuTermApp::new(config, config_path, proxy)?;
    event_loop.run_app(&mut app).context("run event loop")?;
    Ok(())
}

enum OverlayKind {
    RenameTab,
    CommandHelp,
}

struct OverlayState {
    kind: OverlayKind,
    input: String,
    body: String,
    busy: bool,
}

struct NeuTermApp {
    config: Config,
    config_path: Option<PathBuf>,
    mux: Mux,
    triggers: TriggerEngine,
    renderer: Renderer,
    window: Option<Arc<Window>>,
    surface: Option<Surface<Arc<Window>, Arc<Window>>>,
    sb_context: Option<SbContext<Arc<Window>>>,
    modifiers: winit::keyboard::ModifiersState,
    overlay: Option<OverlayState>,
    ai: OllamaClient,
    chat: ChatSession,
    last_frame: Instant,
    needs_redraw: bool,
    runtime: tokio::runtime::Runtime,
    cursor_pos: PhysicalPosition<f64>,
    blink_on: bool,
    last_blink: Instant,
    proxy: EventLoopProxy<AppEvent>,
    selecting: bool,
    selection: Option<Selection>,
    clipboard: Option<arboard::Clipboard>,
}

impl NeuTermApp {
    fn new(
        config: Config,
        config_path: Option<PathBuf>,
        proxy: EventLoopProxy<AppEvent>,
    ) -> Result<Self> {
        let mut mux = Mux::new(&config);
        let wake_proxy = proxy.clone();
        mux.set_wake(Arc::new(move || {
            let _ = wake_proxy.send_event(AppEvent::PtyOutput);
        }));
        mux.bootstrap()?;

        let triggers = TriggerEngine::from_config(&config.triggers);
        let renderer = Renderer::new(&config);
        let ai = match OllamaClient::from_config(&config) {
            Ok(c) => c,
            Err(err) => {
                warn!("AI client init failed ({err}); AI disabled for this session");
                let mut disabled = config.clone();
                disabled.ai.enabled = false;
                OllamaClient::from_config(&disabled).expect("disabled AI client")
            }
        };
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .context("tokio runtime")?;

        Ok(Self {
            config,
            config_path,
            mux,
            triggers,
            renderer,
            window: None,
            surface: None,
            sb_context: None,
            modifiers: Default::default(),
            overlay: None,
            ai,
            chat: ChatSession::default(),
            last_frame: Instant::now(),
            needs_redraw: true,
            runtime,
            cursor_pos: PhysicalPosition::new(0.0, 0.0),
            blink_on: true,
            last_blink: Instant::now(),
            proxy,
            selecting: false,
            selection: None,
            clipboard: arboard::Clipboard::new().ok(),
        })
    }

    fn initial_window_size(&self) -> LogicalSize<f64> {
        let (cw, ch) = self.renderer.cell_size();
        let pad = &self.config.window.padding;
        let cols = self.config.window.initial_cols as f64;
        let rows = self.config.window.initial_rows as f64;
        let w = cols * cw as f64 + (pad.left + pad.right) as f64;
        let h = rows * ch as f64
            + (pad.top + pad.bottom) as f64
            + self.renderer.tab_bar_height() as f64;
        LogicalSize::new(w, h)
    }

    fn handle_action(&mut self, action: Action) {
        match action {
            Action::TabPrev => self.mux.tab_prev(),
            Action::TabNext => self.mux.tab_next(),
            Action::TabMoveLeft => self.mux.tab_move_left(),
            Action::TabMoveRight => self.mux.tab_move_right(),
            Action::TabNew => {
                if let Err(err) = self.mux.new_tab() {
                    error!("new tab: {err}");
                }
            }
            Action::TabClose => {
                if !self.mux.close_active_tab() {
                    std::process::exit(0);
                }
            }
            Action::TabRename => {
                self.overlay = Some(OverlayState {
                    kind: OverlayKind::RenameTab,
                    input: String::new(),
                    body: "Leave blank to reset to auto title".into(),
                    busy: false,
                });
            }
            Action::PaneSplitHorizontal => {
                if let Err(err) = self.mux.split_active(SplitDirection::Horizontal) {
                    error!("split: {err}");
                }
            }
            Action::PaneSplitVertical => {
                if let Err(err) = self.mux.split_active(SplitDirection::Vertical) {
                    error!("split: {err}");
                }
            }
            Action::PaneFocusLeft => self.mux.focus_direction(FocusDir::Left),
            Action::PaneFocusRight => self.mux.focus_direction(FocusDir::Right),
            Action::PaneFocusUp => self.mux.focus_direction(FocusDir::Up),
            Action::PaneFocusDown => self.mux.focus_direction(FocusDir::Down),
            Action::AiCommandHelp => {
                if !self.config.ai.enabled {
                    self.overlay = Some(OverlayState {
                        kind: OverlayKind::CommandHelp,
                        input: String::new(),
                        body: "AI disabled — set ai.enabled: true in config.yaml".into(),
                        busy: false,
                    });
                } else {
                    self.overlay = Some(OverlayState {
                        kind: OverlayKind::CommandHelp,
                        input: String::new(),
                        body: format!("Ask for a command (model {})", self.config.ai.ollama.model),
                        busy: false,
                    });
                }
            }
            Action::AiChatToggle => {
                if let Err(err) = self.mux.open_ai_chat(&self.config.ai.chat.title) {
                    error!("ai chat: {err}");
                }
            }
            Action::ReloadConfig => self.reload_config(),
            Action::Copy => self.copy_selection(),
            Action::Paste => self.paste_clipboard(),
        }
        self.needs_redraw = true;
    }

    fn copy_selection(&mut self) {
        let Some(sel) = self.selection else {
            return;
        };
        let Some(pane) = self.mux.active_pane_mut() else {
            return;
        };
        let Some(session) = &pane.session else {
            return;
        };
        let snap = session.state().snapshot();
        let (r1, c1, r2, c2) = sel.normalized();
        let text = snap.text_range(c1, r1, c2, r2);
        if text.is_empty() {
            return;
        }
        match self.clipboard.as_mut() {
            Some(cb) => {
                if let Err(err) = cb.set_text(text) {
                    error!("clipboard copy failed: {err}");
                } else {
                    info!("copied selection to clipboard");
                }
            }
            None => warn!("clipboard unavailable"),
        }
    }

    fn paste_clipboard(&mut self) {
        let text = match self.clipboard.as_mut() {
            Some(cb) => match cb.get_text() {
                Ok(t) => t,
                Err(err) => {
                    error!("clipboard paste failed: {err}");
                    return;
                }
            },
            None => {
                warn!("clipboard unavailable");
                return;
            }
        };
        // Bracketed paste would be nicer later; for now write raw bytes.
        if let Err(err) = self.mux.write_to_active(text.as_bytes()) {
            error!("paste write failed: {err}");
        }
    }

    fn reload_config(&mut self) {
        let path = match &self.config_path {
            Some(p) => p.clone(),
            None => {
                warn!("no config path to reload");
                return;
            }
        };
        match neuterm_config::load_from_path(&path) {
            Ok(cfg) => {
                info!("config reloaded from {}", path.display());
                self.triggers.reload(&cfg.triggers);
                self.renderer.reload_theme(&cfg);
                if let Err(err) = self.ai.reload(&cfg) {
                    warn!("ai reload: {err}");
                }
                self.config = cfg;
                self.needs_redraw = true;
            }
            Err(err) => error!("config reload failed (keeping previous): {err}"),
        }
    }

    fn confirm_overlay(&mut self) {
        let Some(mut ov) = self.overlay.take() else {
            return;
        };
        match ov.kind {
            OverlayKind::RenameTab => {
                let title = ov.input.trim().to_string();
                self.mux
                    .set_tab_title(if title.is_empty() { None } else { Some(title) });
            }
            OverlayKind::CommandHelp => {
                if !self.config.ai.enabled {
                    self.overlay = Some(ov);
                    return;
                }
                if ov.busy {
                    self.overlay = Some(ov);
                    return;
                }
                if ov.body.starts_with("Suggested:") {
                    if let Some(cmd) =
                        ov.body.lines().next().and_then(|l| {
                            l.strip_prefix("Suggested:").map(|s| s.trim().to_string())
                        })
                    {
                        let mut seq = vec![0x15]; // Ctrl+U
                        seq.extend(cmd.into_bytes());
                        let _ = self.mux.write_to_active(&seq);
                    }
                    return;
                }
                let question = ov.input.clone();
                if question.is_empty() {
                    self.overlay = Some(ov);
                    return;
                }
                ov.busy = true;
                ov.body = "Thinking…".into();
                self.overlay = Some(ov);

                let ai = self.ai.clone();
                let prompt = self.config.ai.command_help.system_prompt.clone();
                let os_hint = std::env::consts::OS;
                let shell_hint =
                    self.config.term.shell.clone().unwrap_or_else(|| {
                        std::env::var("SHELL").unwrap_or_else(|_| "bash".into())
                    });

                let result = self.runtime.block_on(async {
                    ai.suggest_command(&question, os_hint, &shell_hint, prompt.as_deref())
                        .await
                });

                if let Some(ov) = self.overlay.as_mut() {
                    ov.busy = false;
                    match result {
                        Ok(suggestion) => {
                            ov.body = format!(
                                "Suggested: {}\n{}",
                                suggestion.command, suggestion.explanation
                            );
                            ov.input.clear();
                        }
                        Err(err) => {
                            ov.body = format!("Error: {err}");
                        }
                    }
                }
            }
        }
        self.needs_redraw = true;
    }

    fn pump_pty(&mut self) {
        let pump = self.mux.pump_events();
        if pump.bell {
            match self.config.bell.mode {
                neuterm_config::BellMode::Pulse
                | neuterm_config::BellMode::Visual
                | neuterm_config::BellMode::Both => {
                    self.renderer.trigger_bell_pulse();
                }
                _ => {}
            }
            self.needs_redraw = true;
        }
        if pump.dirty {
            self.needs_redraw = true;
        }
    }

    fn redraw(&mut self) {
        let (Some(window), Some(surface)) = (&self.window, &mut self.surface) else {
            return;
        };
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }

        let dt = self.last_frame.elapsed().as_secs_f32();
        self.last_frame = Instant::now();
        self.renderer.tick_pulse(dt);
        self.renderer.set_cursor_blink(self.blink_on);

        let Some(w) = NonZeroU32::new(size.width) else {
            return;
        };
        let Some(h) = NonZeroU32::new(size.height) else {
            return;
        };
        if let Err(err) = surface.resize(w, h) {
            error!("surface resize: {err}");
            return;
        }

        let mut buffer = match surface.buffer_mut() {
            Ok(b) => b,
            Err(err) => {
                error!("buffer: {err}");
                return;
            }
        };

        let mut rgba = vec![0u8; (size.width * size.height * 4) as usize];
        let overlay_view = self.overlay.as_ref().map(|ov| {
            let title = match ov.kind {
                OverlayKind::RenameTab => "Rename tab",
                OverlayKind::CommandHelp => "Command help",
            };
            OverlayView {
                title,
                input: &ov.input,
                body: &ov.body,
            }
        });

        self.renderer.draw(
            &mut rgba,
            size.width,
            size.height,
            &self.mux,
            &self.config,
            &self.triggers,
            overlay_view.as_ref(),
            self.selection,
        );

        for (dst, chunk) in buffer.iter_mut().zip(rgba.chunks_exact(4)) {
            let r = chunk[0] as u32;
            let g = chunk[1] as u32;
            let b = chunk[2] as u32;
            *dst = (r << 16) | (g << 8) | b;
        }

        if let Err(err) = buffer.present() {
            error!("present: {err}");
        }
        self.needs_redraw = false;
    }

    fn on_resize(&mut self, width: u32, height: u32) {
        let (cols, rows) = self.renderer.cols_rows_for_size(width, height);
        // rows from helper already excludes tab bar; mux adds chrome itself historically —
        // pass the visible terminal rows directly.
        self.mux.resize_all(cols, rows.max(2));
        self.needs_redraw = true;
    }

    fn handle_click(&mut self) {
        // Hit regions are recorded in physical pixels (same space as CursorMoved).
        match self.renderer.hit_test(self.cursor_pos.x, self.cursor_pos.y) {
            Some(HitTarget::NewTab) => {
                if let Err(err) = self.mux.new_tab() {
                    error!("new tab: {err}");
                }
                self.needs_redraw = true;
            }
            Some(HitTarget::Tab(i)) => {
                self.mux.activate_tab(i);
                self.needs_redraw = true;
            }
            None => {}
        }
    }
}

impl ApplicationHandler<AppEvent> for NeuTermApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("NeuTerm")
            .with_inner_size(self.initial_window_size());
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        let sb_context = SbContext::new(window.clone()).expect("softbuffer context");
        let surface = Surface::new(&sb_context, window.clone()).expect("softbuffer surface");

        // Sync PTY size to the actual window before first paint.
        let size = window.inner_size();
        self.on_resize(size.width, size.height);

        self.sb_context = Some(sb_context);
        self.surface = Some(surface);
        self.window = Some(window);
        self.needs_redraw = true;
        // Keep proxy alive / referenced so clippy doesn't complain in some builds.
        let _ = &self.proxy;
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::PtyOutput => {
                self.pump_pty();
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                self.pump_pty();
                self.redraw();
            }
            WindowEvent::Resized(size) => {
                self.on_resize(size.width, size.height);
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::ModifiersChanged(m) => {
                self.modifiers = m.state();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_pos = position;
                if self.selecting {
                    if let Some((col, row)) = self.renderer.pixel_to_cell(position.x, position.y) {
                        if let Some(sel) = &mut self.selection {
                            sel.end_col = col;
                            sel.end_row = row;
                            self.needs_redraw = true;
                        }
                    }
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let Some(ov) = self.overlay.as_mut() {
                    if event.state == ElementState::Pressed {
                        match &event.logical_key {
                            Key::Named(NamedKey::Escape) => {
                                self.overlay = None;
                                self.needs_redraw = true;
                            }
                            Key::Named(NamedKey::Enter) => {
                                self.confirm_overlay();
                            }
                            Key::Named(NamedKey::Backspace) => {
                                ov.input.pop();
                                self.needs_redraw = true;
                            }
                            Key::Named(NamedKey::Space)
                                if !self.modifiers.control_key() && !self.modifiers.super_key() =>
                            {
                                // Linux/XKB reports Space as NamedKey::Space.
                                ov.input.push(' ');
                                self.needs_redraw = true;
                            }
                            Key::Character(c)
                                if !self.modifiers.control_key() && !self.modifiers.super_key() =>
                            {
                                ov.input.push_str(c);
                                self.needs_redraw = true;
                            }
                            _ => {}
                        }
                    }
                    return;
                }

                if let Some(action) = action_from_key(&event, self.modifiers) {
                    self.handle_action(action);
                    return;
                }
                if let Some(bytes) = key_to_pty_bytes(&event, self.modifiers) {
                    let is_chat = self
                        .mux
                        .active_pane_mut()
                        .map(|p| p.kind == PaneKind::AiChat)
                        .unwrap_or(false);
                    if !is_chat {
                        if let Err(err) = self.mux.write_to_active(&bytes) {
                            error!("pty write: {err}");
                        }
                        // Optimistic redraw; PTY echo will also wake us.
                        self.needs_redraw = true;
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                    } else if event.state == ElementState::Pressed {
                        if let Key::Character(c) = &event.logical_key {
                            self.chat.push_user(c.as_str());
                        }
                    }
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                match state {
                    ElementState::Pressed => {
                        // Tab bar hits take priority.
                        if self
                            .renderer
                            .hit_test(self.cursor_pos.x, self.cursor_pos.y)
                            .is_some()
                        {
                            self.handle_click();
                        } else if let Some((col, row)) = self
                            .renderer
                            .pixel_to_cell(self.cursor_pos.x, self.cursor_pos.y)
                        {
                            self.selecting = true;
                            self.selection = Some(Selection {
                                start_col: col,
                                start_row: row,
                                end_col: col,
                                end_row: row,
                            });
                            self.needs_redraw = true;
                        } else {
                            self.selection = None;
                            self.needs_redraw = true;
                        }
                    }
                    ElementState::Released => {
                        self.selecting = false;
                    }
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Cursor blink ~530ms half-period
        if self.last_blink.elapsed() >= Duration::from_millis(530) {
            self.blink_on = !self.blink_on;
            self.last_blink = Instant::now();
            self.needs_redraw = true;
        }

        self.pump_pty();

        if self.needs_redraw || self.renderer.is_pulsing() {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }

        // Wake often enough for blink + catch any missed PTY polls.
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(16),
        ));
    }
}
