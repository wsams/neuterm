//! Fast CPU monospace renderer using fontdue.
//!
//! Every character is placed on a fixed cell grid so columns stay aligned.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use fontdue::Font;
use neuterm_config::{Config, Theme};
use neuterm_mux::{Mux, PaneKind, PaneNode};
use neuterm_triggers::{parse_hex_color, TriggerEngine};
use tracing::info;

#[derive(Clone)]
struct GlyphBitmap {
    width: u32,
    height: u32,
    /// Coverage 0–255
    alpha: Vec<u8>,
    /// Offset from cell top-left to bitmap top-left.
    xoff: i32,
    yoff: i32,
}

#[derive(Debug, Clone, Copy)]
pub enum HitTarget {
    Tab(usize),
    NewTab,
}

#[derive(Debug, Clone)]
struct HitRect {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    target: HitTarget,
}

/// Inclusive cell selection in the active terminal pane (grid coords).
#[derive(Debug, Clone, Copy)]
pub struct Selection {
    pub start_col: u16,
    pub start_row: u16,
    pub end_col: u16,
    pub end_row: u16,
}

impl Selection {
    pub fn normalized(self) -> (u16, u16, u16, u16) {
        let (r1, c1, r2, c2) = if (self.start_row, self.start_col) <= (self.end_row, self.end_col) {
            (self.start_row, self.start_col, self.end_row, self.end_col)
        } else {
            (self.end_row, self.end_col, self.start_row, self.start_col)
        };
        (r1, c1, r2, c2)
    }

    pub fn contains(&self, col: u16, row: u16) -> bool {
        let (r1, c1, r2, c2) = self.normalized();
        if row < r1 || row > r2 {
            return false;
        }
        if r1 == r2 {
            return col >= c1 && col <= c2;
        }
        if row == r1 {
            return col >= c1;
        }
        if row == r2 {
            return col <= c2;
        }
        true
    }
}

pub struct ContentLayout {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub cell_w: u32,
    pub cell_h: u32,
}

pub struct Renderer {
    font: Arc<Font>,
    font_bold: Arc<Font>,
    glyph_cache: HashMap<(char, bool), GlyphBitmap>,
    cell_w: u32,
    cell_h: u32,
    baseline: i32,
    tab_bar_h: u32,
    theme: Theme,
    ansi: [[u8; 3]; 16],
    fg: [u8; 3],
    bg: [u8; 3],
    cursor: [u8; 3],
    split: [u8; 3],
    selection_bg: [u8; 3],
    selection_fg: [u8; 3],
    focus_border: [u8; 3],
    font_family: String,
    font_size: f32,
    pulse: f32,
    hit_regions: Vec<HitRect>,
    cursor_blink_on: bool,
    content_layout: ContentLayout,
}

impl Renderer {
    pub fn new(config: &Config) -> Self {
        let theme = config.resolved_theme();
        let (font, font_bold, resolved) = load_fonts(&config.font.family);
        info!(
            "using font '{resolved}' at {}px; theme '{}'",
            config.font.size, theme.name
        );
        let (cell_w, cell_h, baseline) = measure_cell(&font, config.font.size);
        let mut renderer = Self {
            font: Arc::new(font),
            font_bold: Arc::new(font_bold),
            glyph_cache: HashMap::new(),
            cell_w,
            cell_h,
            baseline,
            tab_bar_h: theme.tabs.bar_height(cell_h),
            theme,
            ansi: [[0; 3]; 16],
            fg: [0xa5, 0xa7, 0xaa],
            bg: [0x25, 0x28, 0x2e],
            cursor: [0xa5, 0xa6, 0xaa],
            split: [0xa5, 0xa7, 0xaa],
            selection_bg: [0x7d, 0x7d, 0x7d],
            selection_fg: [0, 0, 0],
            focus_border: [0xa5, 0xa6, 0xaa],
            font_family: resolved,
            font_size: config.font.size,
            pulse: 0.0,
            hit_regions: Vec::new(),
            cursor_blink_on: true,
            content_layout: ContentLayout {
                x: 0,
                y: 0,
                w: 0,
                h: 0,
                cell_w,
                cell_h,
            },
        };
        renderer.apply_theme_colors();
        renderer
    }

    pub fn reload_theme(&mut self, config: &Config) {
        self.theme = config.resolved_theme();
        self.apply_theme_colors();
        self.tab_bar_h = self.theme.tabs.bar_height(self.cell_h);
        if (config.font.size - self.font_size).abs() > 0.01
            || config.font.family != self.font_family
        {
            let (font, font_bold, resolved) = load_fonts(&config.font.family);
            let (cw, ch, baseline) = measure_cell(&font, config.font.size);
            self.font = Arc::new(font);
            self.font_bold = Arc::new(font_bold);
            self.font_family = resolved;
            self.font_size = config.font.size;
            self.cell_w = cw;
            self.cell_h = ch;
            self.baseline = baseline;
            self.tab_bar_h = self.theme.tabs.bar_height(ch);
            self.glyph_cache.clear();
        }
    }

    fn apply_theme_colors(&mut self) {
        let c = &self.theme.colors;
        self.ansi = parse_ansi_palette(c);
        self.fg = parse_rgb(&c.foreground).unwrap_or(self.fg);
        self.bg = parse_rgb(&c.background).unwrap_or(self.bg);
        self.cursor = parse_rgb(&c.cursor).unwrap_or(self.cursor);
        self.split = parse_rgb(&c.split).unwrap_or(self.split);
        self.selection_bg = parse_rgb(&c.selection_bg).unwrap_or(self.selection_bg);
        self.selection_fg = parse_rgb(&c.selection_fg).unwrap_or(self.selection_fg);
        self.focus_border = parse_rgb(&self.theme.panes.focus_border).unwrap_or(self.cursor);
    }

    pub fn cell_size(&self) -> (f32, f32) {
        (self.cell_w as f32, self.cell_h as f32)
    }

    pub fn cell_size_px(&self) -> (u32, u32) {
        (self.cell_w, self.cell_h)
    }

    pub fn tab_bar_height(&self) -> u32 {
        self.tab_bar_h
    }

    pub fn content_layout(&self) -> &ContentLayout {
        &self.content_layout
    }

    pub fn trigger_bell_pulse(&mut self) {
        self.pulse = 1.0;
    }

    pub fn tick_pulse(&mut self, dt: f32) {
        if self.pulse > 0.0 {
            self.pulse = (self.pulse - dt * 3.0).max(0.0);
        }
    }

    pub fn is_pulsing(&self) -> bool {
        self.pulse > 0.0
    }

    pub fn set_cursor_blink(&mut self, on: bool) {
        self.cursor_blink_on = on;
    }

    pub fn hit_test(&self, x: f64, y: f64) -> Option<HitTarget> {
        let x = x as u32;
        let y = y as u32;
        for region in self.hit_regions.iter().rev() {
            if x >= region.x && y >= region.y && x < region.x + region.w && y < region.y + region.h
            {
                return Some(region.target);
            }
        }
        None
    }

    /// Convert a window pixel position to terminal cell coords, if inside content.
    pub fn pixel_to_cell(&self, x: f64, y: f64) -> Option<(u16, u16)> {
        let layout = &self.content_layout;
        if layout.cell_w == 0 || layout.cell_h == 0 {
            return None;
        }
        let x = x as i64;
        let y = y as i64;
        if x < layout.x as i64
            || y < layout.y as i64
            || x >= (layout.x + layout.w) as i64
            || y >= (layout.y + layout.h) as i64
        {
            return None;
        }
        let col = ((x - layout.x as i64) / layout.cell_w as i64) as u16;
        let row = ((y - layout.y as i64) / layout.cell_h as i64) as u16;
        Some((col, row))
    }

    pub fn cols_rows_for_size(&self, width: u32, height: u32) -> (u16, u16) {
        let pad = &self.theme.window_padding;
        let inset = &self.theme.pane_inset;
        let avail_w = width.saturating_sub(inset.left + inset.right + pad.left + pad.right);
        let avail_h =
            height.saturating_sub(self.tab_bar_h + inset.top + inset.bottom + pad.top + pad.bottom);
        let cols = (avail_w / self.cell_w.max(1)).max(1) as u16;
        let rows = (avail_h / self.cell_h.max(1)).max(1) as u16;
        (cols, rows)
    }

    pub fn draw(
        &mut self,
        pixels: &mut [u8],
        width: u32,
        height: u32,
        mux: &Mux,
        config: &Config,
        triggers: &TriggerEngine,
        overlay: Option<&OverlayView>,
        selection: Option<Selection>,
    ) {
        self.hit_regions.clear();
        fill(pixels, width, height, self.bg);

        self.draw_tab_bar(pixels, width, mux, config);

        let pad = &self.theme.window_padding;
        let inset = &self.theme.pane_inset;
        // Pane chrome (focus border sits on this rect). Top inset is usually 0
        // so the tab bar sits flush on the content frame.
        let pane_x = inset.left;
        let pane_y = self.tab_bar_h + inset.top;
        let pane_w = width.saturating_sub(inset.left + inset.right);
        let pane_h = height.saturating_sub(self.tab_bar_h + inset.top + inset.bottom);

        // Terminal cells sit directly inside the themed window padding.
        let grid_x = pane_x + pad.left;
        let grid_y = pane_y + pad.top;
        let grid_w = pane_w.saturating_sub(pad.left + pad.right);
        let grid_h = pane_h.saturating_sub(pad.top + pad.bottom);

        self.content_layout = ContentLayout {
            x: grid_x,
            y: grid_y,
            w: grid_w,
            h: grid_h,
            cell_w: self.cell_w,
            cell_h: self.cell_h,
        };

        if let Some(tab) = mux.active_tab() {
            self.draw_pane_tree(
                pixels,
                width,
                height,
                mux,
                triggers,
                &tab.tree,
                tab.active_pane,
                pane_x,
                pane_y,
                pane_w,
                pane_h,
                config,
                selection,
            );
        }

        if self.pulse > 0.0 {
            let glow = [
                self.cursor[0],
                self.cursor[1],
                (self.cursor[2] as f32 * 0.6) as u8,
            ];
            blend_rect(
                pixels,
                width,
                height,
                pane_x,
                pane_y,
                pane_w,
                pane_h,
                glow,
                self.pulse * 0.25,
            );
        }

        if let Some(ov) = overlay {
            self.draw_overlay(pixels, width, height, ov);
        }
    }

    fn draw_tab_bar(&mut self, pixels: &mut [u8], width: u32, mux: &Mux, config: &Config) {
        if !config.tabs.show_bar {
            return;
        }
        let chrome = self.theme.tabs.clone();
        let bar_bg = parse_rgb(&chrome.bar_background).unwrap_or(darken(self.bg, 0.85));
        let active_bg = parse_rgb(&chrome.active_background).unwrap_or(lighten(self.bg, 1.2));
        let inactive_bg = parse_rgb(&chrome.inactive_background).unwrap_or(bar_bg);
        let active_fg = parse_rgb(&chrome.active_foreground).unwrap_or(self.fg);
        let inactive_fg = parse_rgb(&chrome.inactive_foreground).unwrap_or(darken(self.fg, 0.7));
        let sep = parse_rgb(&chrome.separator_color).unwrap_or(self.split);
        let active_shadow = darken(active_bg, 0.55);
        let active_highlight = lighten(active_bg, 1.18);
        let plus_bg = mix_rgb(self.focus_border, active_bg, 0.72);
        let plus_fg = pick_contrast(plus_bg, self.bg, [245, 247, 250]);
        let plus_shadow = darken(plus_bg, 0.45);

        let bar_h = self.tab_bar_h;
        fill_rect(pixels, width, bar_h, 0, 0, width, bar_h, bar_bg);

        let tab_h = chrome.tab_height(self.cell_h);
        let tab_y = chrome.bar_padding.top;
        let mut x = chrome.bar_padding.left;

        for (i, tab) in mux.tabs.iter().enumerate() {
            let title = tab.title(&mux.panes);
            let label = if tab.title_override.is_some() {
                format!("*{title}")
            } else {
                title
            };
            let active = i == mux.active_tab;
            let label_w = label.chars().count() as u32 * self.cell_w;
            let tw = label_w + chrome.tab_padding.left + chrome.tab_padding.right;
            let tw = tw
                .max(tab_h)
                .min(width.saturating_sub(x + tab_h + chrome.gap + chrome.bar_padding.right));

            if active {
                draw_shadow(
                    pixels,
                    width,
                    bar_h,
                    x,
                    tab_y + 2,
                    tw,
                    tab_h,
                    active_shadow,
                    0.35,
                );
            }
            fill_rect(
                pixels,
                width,
                bar_h,
                x,
                tab_y,
                tw,
                tab_h,
                if active { active_bg } else { inactive_bg },
            );
            if active && tw > 2 {
                fill_rect(
                    pixels,
                    width,
                    bar_h,
                    x + 1,
                    tab_y,
                    tw - 2,
                    1,
                    active_highlight,
                );
            }
            // Vertically center glyph row inside the chip using even tab_padding.
            let text_x = x + chrome.tab_padding.left;
            let text_y = tab_y + chrome.tab_padding.top;
            self.draw_text(
                pixels,
                width,
                bar_h,
                text_x,
                text_y,
                &label,
                if active { active_fg } else { inactive_fg },
            );
            self.hit_regions.push(HitRect {
                x,
                y: tab_y,
                w: tw,
                h: tab_h,
                target: HitTarget::Tab(i),
            });
            x += tw + chrome.gap;
        }

        // + button: same height as tabs, square, evenly padded glyph.
        let plus_size = tab_h;
        let plus_x = x;
        draw_shadow(
            pixels,
            width,
            bar_h,
            plus_x,
            tab_y + 2,
            plus_size,
            plus_size,
            plus_shadow,
            0.28,
        );
        fill_rect(
            pixels, width, bar_h, plus_x, tab_y, plus_size, plus_size, plus_bg,
        );
        if plus_size > 2 {
            fill_rect(
                pixels,
                width,
                bar_h,
                plus_x + 1,
                tab_y,
                plus_size - 2,
                1,
                lighten(plus_bg, 1.18),
            );
        }
        let icon_thickness = (plus_size / 10).max(2);
        let icon_len = (plus_size / 2).max(icon_thickness * 3);
        let cx = plus_x + plus_size / 2;
        let cy = tab_y + plus_size / 2;
        fill_rect(
            pixels,
            width,
            bar_h,
            cx.saturating_sub(icon_len / 2),
            cy.saturating_sub(icon_thickness / 2),
            icon_len,
            icon_thickness,
            plus_fg,
        );
        fill_rect(
            pixels,
            width,
            bar_h,
            cx.saturating_sub(icon_thickness / 2),
            cy.saturating_sub(icon_len / 2),
            icon_thickness,
            icon_len,
            plus_fg,
        );
        self.hit_regions.push(HitRect {
            x: plus_x,
            y: tab_y,
            w: plus_size,
            h: plus_size,
            target: HitTarget::NewTab,
        });

        if chrome.separator_height > 0 {
            fill_rect(
                pixels,
                width,
                bar_h,
                0,
                bar_h.saturating_sub(chrome.separator_height),
                width,
                chrome.separator_height,
                sep,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_pane_tree(
        &mut self,
        pixels: &mut [u8],
        width: u32,
        height: u32,
        mux: &Mux,
        triggers: &TriggerEngine,
        node: &PaneNode,
        active_pane: neuterm_mux::PaneId,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        config: &Config,
        selection: Option<Selection>,
    ) {
        match node {
            PaneNode::Leaf(id) => {
                let focused = *id == active_pane;
                let pad = &self.theme.window_padding;
                let grid_x = x + pad.left;
                let grid_y = y + pad.top;
                let grid_w = w.saturating_sub(pad.left + pad.right);
                let grid_h = h.saturating_sub(pad.top + pad.bottom);

                // Fill pane background out to the border.
                fill_rect(pixels, width, height, x, y, w, h, self.bg);

                if let Some(pane) = mux.panes.get(id) {
                    match pane.kind {
                        PaneKind::Terminal => {
                            if let Some(session) = &pane.session {
                                let snap = session.state().snapshot();
                                let sel = if focused { selection } else { None };
                                if focused {
                                    self.content_layout = ContentLayout {
                                        x: grid_x,
                                        y: grid_y,
                                        w: grid_w,
                                        h: grid_h,
                                        cell_w: self.cell_w,
                                        cell_h: self.cell_h,
                                    };
                                }
                                self.draw_grid(
                                    pixels, width, height, &snap, triggers, grid_x, grid_y, grid_w,
                                    grid_h, focused, sel,
                                );
                            }
                        }
                        PaneKind::AiChat => {
                            self.draw_text(
                                pixels,
                                width,
                                height,
                                grid_x,
                                grid_y,
                                "AI Chat — set ai.enabled: true",
                                self.fg,
                            );
                        }
                    }
                }
                if focused {
                    // Avoid outlining the outer window perimeter; reserve the focus
                    // stroke for interior split edges only.
                    let flush_under_tabs = self.theme.pane_inset.top == 0
                        && self.theme.tabs.separator_height > 0
                        && y <= self.tab_bar_h;
                    let root_x = self.theme.pane_inset.left;
                    let root_y = self.tab_bar_h + self.theme.pane_inset.top;
                    let root_w = width
                        .saturating_sub(self.theme.pane_inset.left + self.theme.pane_inset.right);
                    let root_h = height.saturating_sub(
                        self.tab_bar_h + self.theme.pane_inset.top + self.theme.pane_inset.bottom,
                    );
                    stroke_rect_sides(
                        pixels,
                        width,
                        height,
                        x,
                        y,
                        w,
                        h,
                        self.focus_border,
                        1,
                        !flush_under_tabs && y > root_y, // top
                        y + h < root_y + root_h,         // bottom
                        x > root_x,                      // left
                        x + w < root_x + root_w,         // right
                    );
                }
            }
            PaneNode::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let ratio = ratio.clamp(0.1, 0.9);
                match direction {
                    neuterm_mux::SplitDirection::Horizontal => {
                        let left_w = ((w as f32) * ratio) as u32;
                        let right_w = w.saturating_sub(left_w + 2);
                        self.draw_pane_tree(
                            pixels,
                            width,
                            height,
                            mux,
                            triggers,
                            first,
                            active_pane,
                            x,
                            y,
                            left_w,
                            h,
                            config,
                            selection,
                        );
                        fill_rect(pixels, width, height, x + left_w, y, 2, h, self.split);
                        self.draw_pane_tree(
                            pixels,
                            width,
                            height,
                            mux,
                            triggers,
                            second,
                            active_pane,
                            x + left_w + 2,
                            y,
                            right_w,
                            h,
                            config,
                            selection,
                        );
                    }
                    neuterm_mux::SplitDirection::Vertical => {
                        let top_h = ((h as f32) * ratio) as u32;
                        let bot_h = h.saturating_sub(top_h + 2);
                        self.draw_pane_tree(
                            pixels,
                            width,
                            height,
                            mux,
                            triggers,
                            first,
                            active_pane,
                            x,
                            y,
                            w,
                            top_h,
                            config,
                            selection,
                        );
                        fill_rect(pixels, width, height, x, y + top_h, w, 2, self.split);
                        self.draw_pane_tree(
                            pixels,
                            width,
                            height,
                            mux,
                            triggers,
                            second,
                            active_pane,
                            x,
                            y + top_h + 2,
                            w,
                            bot_h,
                            config,
                            selection,
                        );
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_grid(
        &mut self,
        pixels: &mut [u8],
        width: u32,
        height: u32,
        snap: &neuterm_term::GridSnapshot,
        triggers: &TriggerEngine,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        focused: bool,
        selection: Option<Selection>,
    ) {
        let dim = if focused {
            1.0
        } else {
            1.0 - self.theme.panes.inactive_dim.clamp(0.0, 0.8)
        };

        fill_rect(pixels, width, height, x, y, w, h, scale_rgb(self.bg, dim));

        let rows = snap.rows.min((h / self.cell_h.max(1)) as u16);
        let cols = snap.cols.min((w / self.cell_w.max(1)) as u16);
        let cell_w = self.cell_w;
        let cell_h = self.cell_h;

        for row in 0..rows as usize {
            let mut line = String::with_capacity(cols as usize);
            for col in 0..cols as usize {
                line.push(snap.cells[row * snap.cols as usize + col].ch);
            }
            let decs = triggers.decorations_for_line(line.trim_end());

            for col in 0..cols as usize {
                let cell = snap.cells[row * snap.cols as usize + col];
                let mut fg = cell.fg.to_rgb(&self.ansi, self.fg, self.bg);
                let mut bg = cell.bg.to_rgb(&self.ansi, self.fg, self.bg);
                if cell.attr.inverse {
                    std::mem::swap(&mut fg, &mut bg);
                }
                for d in &decs {
                    if col >= d.start && col < d.end {
                        if let Some(c) = d.style.foreground {
                            fg = [c.r, c.g, c.b];
                        }
                        if let Some(c) = d.style.background {
                            bg = [c.r, c.g, c.b];
                        }
                    }
                }

                let selected = selection
                    .map(|s| s.contains(col as u16, row as u16))
                    .unwrap_or(false);
                if selected {
                    bg = self.selection_bg;
                    fg = self.selection_fg;
                }

                fg = scale_rgb(fg, dim);
                bg = scale_rgb(bg, dim);

                let px = x + col as u32 * cell_w;
                let py = y + row as u32 * cell_h;

                if bg != scale_rgb(self.bg, dim) || selected {
                    fill_rect(pixels, width, height, px, py, cell_w, cell_h, bg);
                }

                if cell.ch != ' ' && cell.ch != '\0' {
                    self.blit_char(pixels, width, height, px, py, cell.ch, fg, cell.attr.bold);
                }
            }
        }

        if focused && self.cursor_blink_on {
            let cx = x + snap.cursor_col as u32 * cell_w;
            let cy = y + snap.cursor_row as u32 * cell_h;
            if snap.cursor_col < cols && snap.cursor_row < rows {
                fill_rect(pixels, width, height, cx, cy, cell_w, cell_h, self.cursor);
                let idx = snap.cursor_row as usize * snap.cols as usize + snap.cursor_col as usize;
                if let Some(cell) = snap.cells.get(idx) {
                    if cell.ch != ' ' && cell.ch != '\0' {
                        self.blit_char(
                            pixels,
                            width,
                            height,
                            cx,
                            cy,
                            cell.ch,
                            self.bg,
                            cell.attr.bold,
                        );
                    }
                }
            }
        }
    }

    fn draw_overlay(&mut self, pixels: &mut [u8], width: u32, height: u32, ov: &OverlayView) {
        let box_w = (width as f32 * 0.6).min(720.0) as u32;
        let box_h = self.cell_h * 6 + 24;
        let x = width.saturating_sub(box_w) / 2;
        let y = height.saturating_sub(box_h) / 2;
        fill_rect(
            pixels,
            width,
            height,
            x,
            y,
            box_w,
            box_h,
            darken(self.bg, 0.7),
        );
        stroke_rect(pixels, width, height, x, y, box_w, box_h, self.cursor, 1);
        self.draw_text(pixels, width, height, x + 16, y + 12, ov.title, self.fg);
        self.draw_text(
            pixels,
            width,
            height,
            x + 16,
            y + 12 + self.cell_h,
            &format!("> {}", ov.input),
            self.fg,
        );
        if !ov.body.is_empty() {
            self.draw_text(
                pixels,
                width,
                height,
                x + 16,
                y + 12 + self.cell_h * 3,
                &ov.body,
                darken(self.fg, 0.85),
            );
        }
    }

    fn draw_text(
        &mut self,
        pixels: &mut [u8],
        width: u32,
        height: u32,
        mut x: u32,
        y: u32,
        text: &str,
        color: [u8; 3],
    ) {
        for ch in text.chars() {
            if ch == '\n' {
                continue;
            }
            self.blit_char(pixels, width, height, x, y, ch, color, false);
            x += self.cell_w;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn blit_char(
        &mut self,
        pixels: &mut [u8],
        width: u32,
        height: u32,
        cell_x: u32,
        cell_y: u32,
        ch: char,
        color: [u8; 3],
        bold: bool,
    ) {
        if !self.glyph_cache.contains_key(&(ch, bold)) {
            let glyph = self.rasterize_glyph(ch, bold);
            self.glyph_cache.insert((ch, bold), glyph);
        }
        let Some(glyph) = self.glyph_cache.get(&(ch, bold)).cloned() else {
            return;
        };
        blit_alpha(
            pixels,
            width,
            height,
            cell_x as i32 + glyph.xoff,
            cell_y as i32 + glyph.yoff,
            glyph.width,
            glyph.height,
            &glyph.alpha,
            color,
        );
    }

    fn rasterize_glyph(&self, ch: char, bold: bool) -> GlyphBitmap {
        let font = if bold { &self.font_bold } else { &self.font };
        let (metrics, bitmap) = font.rasterize(ch, self.font_size);

        // Bake into a fixed cell so every glyph shares the same origin.
        // fontdue metrics are y-up; our framebuffer is y-down.
        // Bitmap row 0 is the top of the glyph:
        //   y = baseline - height - ymin
        let mut alpha = vec![0u8; (self.cell_w * self.cell_h) as usize];
        if metrics.width == 0 || metrics.height == 0 || bitmap.is_empty() {
            return GlyphBitmap {
                width: self.cell_w,
                height: self.cell_h,
                alpha,
                xoff: 0,
                yoff: 0,
            };
        }

        let glyph_x = metrics.xmin;
        let glyph_y = self.baseline - metrics.height as i32 - metrics.ymin;

        for row in 0..metrics.height {
            for col in 0..metrics.width {
                let src = row * metrics.width + col;
                let a = bitmap[src];
                if a < 8 {
                    continue;
                }
                let dx = glyph_x + col as i32;
                let dy = glyph_y + row as i32;
                if dx < 0 || dy < 0 || dx >= self.cell_w as i32 || dy >= self.cell_h as i32 {
                    continue;
                }
                let dst = (dy as u32 * self.cell_w + dx as u32) as usize;
                alpha[dst] = alpha[dst].saturating_add(a);
            }
        }

        GlyphBitmap {
            width: self.cell_w,
            height: self.cell_h,
            alpha,
            xoff: 0,
            yoff: 0,
        }
    }
}

pub struct OverlayView<'a> {
    pub title: &'a str,
    pub input: &'a str,
    pub body: &'a str,
}

fn measure_cell(font: &Font, px: f32) -> (u32, u32, i32) {
    // Monospace: W/i/m advances should match. Use the max of a few samples.
    let mut advance = 0.0f32;
    for ch in ['W', 'M', 'i', ' ', '0'] {
        advance = advance.max(font.metrics(ch, px).advance_width);
    }
    let line = font.horizontal_line_metrics(px);
    let ascent = line.map(|l| l.ascent).unwrap_or(px * 0.8);
    let descent = line.map(|l| l.descent.abs()).unwrap_or(px * 0.2);
    // Pad height slightly so descenders/underlines aren't clipped.
    let cell_w = advance.round().max(1.0) as u32;
    let cell_h = (ascent + descent).ceil().max(px).max(1.0) as u32 + 1;
    let baseline = ascent.round() as i32;
    (cell_w, cell_h, baseline)
}

fn load_fonts(preferred: &str) -> (Font, Font, String) {
    let candidates = font_candidates(preferred);
    for path in &candidates {
        if !is_safe_mono_font_path(path) {
            continue;
        }
        let Ok(data) = std::fs::read(path) else {
            continue;
        };
        let Ok(regular) = Font::from_bytes(data.as_slice(), fontdue::FontSettings::default())
        else {
            continue;
        };
        if !looks_monospace(&regular, 14.0) {
            info!("skipping non-monospace font {}", path.display());
            continue;
        }
        let bold = load_bold_companion(path).unwrap_or_else(|| regular.clone());
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| preferred.to_string());
        return (regular, bold, name);
    }
    panic!(
        "NeuTerm could not load a monospace font. Install liberation-mono-fonts or adwaita-mono-fonts. Tried: {candidates:?}"
    );
}

fn looks_monospace(font: &Font, px: f32) -> bool {
    let w = font.metrics('W', px).advance_width;
    let i = font.metrics('i', px).advance_width;
    let m = font.metrics('m', px).advance_width;
    if w <= 1.0 {
        return false;
    }
    // Allow tiny float error; reject proportional faces hard.
    (w - i).abs() < 0.75 && (w - m).abs() < 0.75
}

fn is_safe_mono_font_path(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    // Variable-font filenames like NotoSans[wght].ttf are poison for terminals.
    if name.contains('[') || name.contains("variable") || name.contains("-vf") {
        return false;
    }
    // Reject obvious UI/sans families unless they claim mono.
    let is_mono = name.contains("mono")
        || name.contains("consolas")
        || name.contains("courier")
        || name.contains("sourcecode")
        || name.contains("firacode")
        || name.contains("jetbrains")
        || name.contains("hack")
        || name.contains("iosevka")
        || name.contains("inconsolata")
        || name.contains("cascadia")
        || name.contains("menlo")
        || name.contains("sfmono")
        || name.contains("adwaita");
    if !is_mono {
        // Still allow if path lives under a *mono* directory.
        let dir = path
            .parent()
            .map(|p| p.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if !(dir.contains("mono") || dir.contains("firacode") || dir.contains("jetbrains")) {
            return false;
        }
    }
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("ttf") | Some("otf")
    )
}

fn load_bold_companion(regular_path: &Path) -> Option<Font> {
    let name = regular_path.file_name()?.to_string_lossy();
    let bold_name = name
        .replace("Regular", "Bold")
        .replace("regular", "Bold")
        .replace("-Medium.", "-Bold.");
    if bold_name == name {
        return None;
    }
    let bold_path = regular_path.with_file_name(bold_name);
    if !is_safe_mono_font_path(&bold_path) {
        return None;
    }
    let data = std::fs::read(bold_path).ok()?;
    Font::from_bytes(data.as_slice(), fontdue::FontSettings::default()).ok()
}

fn font_candidates(preferred: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let preferred_lower = preferred.to_ascii_lowercase().replace(' ', "");

    // Hardcoded trustworthy monospace files first (order = preference).
    let builtins = [
        "/usr/share/fonts/adwaita-mono-fonts/AdwaitaMono-Regular.ttf",
        "/usr/share/fonts/liberation-mono-fonts/LiberationMono-Regular.ttf",
        "/usr/share/fonts/adobe-source-code-pro-fonts/SourceCodePro-Regular.otf",
        "/usr/share/fonts/fira-code/FiraCode-Regular.ttf",
        "/usr/share/fonts/dejavu/DejaVuSansMono.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        "/usr/share/fonts/TTF/JetBrainsMono-Regular.ttf",
        "/usr/share/fonts/jetbrains-mono-fonts/JetBrainsMono-Regular.ttf",
    ];
    for p in builtins {
        let path = PathBuf::from(p);
        if path.exists() {
            // Prefer an exact family match by bubbling it to the front.
            let name = path
                .file_name()
                .map(|s| {
                    s.to_string_lossy()
                        .to_ascii_lowercase()
                        .replace(['-', ' '], "")
                })
                .unwrap_or_default();
            if !preferred_lower.is_empty() && name.contains(&preferred_lower) {
                out.insert(0, path);
            } else {
                out.push(path);
            }
        }
    }

    // fc-match, but only if it returns a safe mono path (never blind-trust it).
    if let Ok(output) = std::process::Command::new("fc-match")
        .args([preferred, ":charset=ascii", "--format=%{file}\n"])
        .output()
    {
        if output.status.success() {
            if let Ok(s) = String::from_utf8(output.stdout) {
                let p = PathBuf::from(s.trim());
                if p.exists() && is_safe_mono_font_path(&p) {
                    out.insert(0, p);
                }
            }
        }
    }
    // Also try forcing monospace spacing via fontconfig.
    if let Ok(output) = std::process::Command::new("fc-match")
        .args(["monospace", "--format=%{file}\n"])
        .output()
    {
        if output.status.success() {
            if let Ok(s) = String::from_utf8(output.stdout) {
                let p = PathBuf::from(s.trim());
                if p.exists() && is_safe_mono_font_path(&p) {
                    out.push(p);
                }
            }
        }
    }

    out
}

fn parse_ansi_palette(colors: &neuterm_config::ColorsConfig) -> [[u8; 3]; 16] {
    let mut out = [[0u8; 3]; 16];
    for (i, c) in colors.ansi.iter().enumerate().take(8) {
        out[i] = parse_rgb(c).unwrap_or([0, 0, 0]);
    }
    for (i, c) in colors.brights.iter().enumerate().take(8) {
        out[8 + i] = parse_rgb(c).unwrap_or([0, 0, 0]);
    }
    out
}

fn parse_rgb(s: &str) -> Option<[u8; 3]> {
    parse_hex_color(s).map(|c| [c.r, c.g, c.b])
}

fn fill(pixels: &mut [u8], width: u32, height: u32, rgb: [u8; 3]) {
    fill_rect(pixels, width, height, 0, 0, width, height, rgb);
}

fn fill_rect(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    rgb: [u8; 3],
) {
    let x1 = x.min(width);
    let y1 = y.min(height);
    let x2 = (x + w).min(width);
    let y2 = (y + h).min(height);
    for py in y1..y2 {
        let row = (py * width) as usize;
        for px in x1..x2 {
            let i = (row + px as usize) * 4;
            if i + 3 < pixels.len() {
                pixels[i] = rgb[0];
                pixels[i + 1] = rgb[1];
                pixels[i + 2] = rgb[2];
                pixels[i + 3] = 255;
            }
        }
    }
}

fn blend_rect(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    rgb: [u8; 3],
    a: f32,
) {
    let a = a.clamp(0.0, 1.0);
    let x1 = x.min(width);
    let y1 = y.min(height);
    let x2 = (x + w).min(width);
    let y2 = (y + h).min(height);
    for py in y1..y2 {
        for px in x1..x2 {
            let i = ((py * width + px) * 4) as usize;
            if i + 3 < pixels.len() {
                pixels[i] = ((pixels[i] as f32) * (1.0 - a) + rgb[0] as f32 * a) as u8;
                pixels[i + 1] = ((pixels[i + 1] as f32) * (1.0 - a) + rgb[1] as f32 * a) as u8;
                pixels[i + 2] = ((pixels[i + 2] as f32) * (1.0 - a) + rgb[2] as f32 * a) as u8;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_shadow(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    rgb: [u8; 3],
    strength: f32,
) {
    blend_rect(pixels, width, height, x, y, w, h, rgb, strength);
    if w > 4 {
        blend_rect(
            pixels,
            width,
            height,
            x + 2,
            y + h,
            w - 4,
            1,
            rgb,
            strength * 0.65,
        );
    }
}

fn stroke_rect(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    rgb: [u8; 3],
    thickness: u32,
) {
    stroke_rect_sides(
        pixels, width, height, x, y, w, h, rgb, thickness, true, true, true, true,
    );
}

fn stroke_rect_sides(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    rgb: [u8; 3],
    thickness: u32,
    top: bool,
    bottom: bool,
    left: bool,
    right: bool,
) {
    if top {
        fill_rect(pixels, width, height, x, y, w, thickness, rgb);
    }
    if bottom {
        fill_rect(
            pixels,
            width,
            height,
            x,
            y + h.saturating_sub(thickness),
            w,
            thickness,
            rgb,
        );
    }
    if left {
        fill_rect(pixels, width, height, x, y, thickness, h, rgb);
    }
    if right {
        fill_rect(
            pixels,
            width,
            height,
            x + w.saturating_sub(thickness),
            y,
            thickness,
            h,
            rgb,
        );
    }
}

fn blit_alpha(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    gw: u32,
    gh: u32,
    alpha: &[u8],
    rgb: [u8; 3],
) {
    if gw == 0 || gh == 0 || alpha.len() < (gw * gh) as usize {
        return;
    }
    for row in 0..gh {
        for col in 0..gw {
            let a = alpha[(row * gw + col) as usize];
            if a < 8 {
                continue;
            }
            let px = x + col as i32;
            let py = y + row as i32;
            if px < 0 || py < 0 || px >= width as i32 || py >= height as i32 {
                continue;
            }
            let i = ((py as u32 * width + px as u32) * 4) as usize;
            if i + 3 >= pixels.len() {
                continue;
            }
            let af = a as f32 / 255.0;
            pixels[i] = ((pixels[i] as f32) * (1.0 - af) + rgb[0] as f32 * af) as u8;
            pixels[i + 1] = ((pixels[i + 1] as f32) * (1.0 - af) + rgb[1] as f32 * af) as u8;
            pixels[i + 2] = ((pixels[i + 2] as f32) * (1.0 - af) + rgb[2] as f32 * af) as u8;
        }
    }
}

fn darken(rgb: [u8; 3], factor: f32) -> [u8; 3] {
    scale_rgb(rgb, factor)
}

fn lighten(rgb: [u8; 3], factor: f32) -> [u8; 3] {
    [
        (rgb[0] as f32 * factor).min(255.0) as u8,
        (rgb[1] as f32 * factor).min(255.0) as u8,
        (rgb[2] as f32 * factor).min(255.0) as u8,
    ]
}

fn mix_rgb(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        (a[0] as f32 * t + b[0] as f32 * (1.0 - t)) as u8,
        (a[1] as f32 * t + b[1] as f32 * (1.0 - t)) as u8,
        (a[2] as f32 * t + b[2] as f32 * (1.0 - t)) as u8,
    ]
}

fn pick_contrast(bg: [u8; 3], dark: [u8; 3], light: [u8; 3]) -> [u8; 3] {
    if luminance(bg) > 140.0 {
        dark
    } else {
        light
    }
}

fn luminance(rgb: [u8; 3]) -> f32 {
    rgb[0] as f32 * 0.299 + rgb[1] as f32 * 0.587 + rgb[2] as f32 * 0.114
}

fn scale_rgb(rgb: [u8; 3], factor: f32) -> [u8; 3] {
    [
        (rgb[0] as f32 * factor).clamp(0.0, 255.0) as u8,
        (rgb[1] as f32 * factor).clamp(0.0, 255.0) as u8,
        (rgb[2] as f32 * factor).clamp(0.0, 255.0) as u8,
    ]
}
