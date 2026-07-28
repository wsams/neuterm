//! Cell grid with scrollback ring buffer.

use crate::color::{Color, NamedColor};

#[derive(Debug, Clone, Copy)]
pub struct CellAttr {
    pub bold: bool,
    pub underline: bool,
    pub inverse: bool,
}

impl Default for CellAttr {
    fn default() -> Self {
        Self {
            bold: false,
            underline: false,
            inverse: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Cell {
    pub ch: char,
    pub fg: Color,
    pub bg: Color,
    pub attr: CellAttr,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: Color::Named(NamedColor::Foreground),
            bg: Color::Named(NamedColor::Background),
            attr: CellAttr::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GridSnapshot {
    pub cols: u16,
    pub rows: u16,
    pub cursor_col: u16,
    pub cursor_row: u16,
    /// Visible rows, row-major.
    pub cells: Vec<Cell>,
}

impl GridSnapshot {
    /// Extract selected text (inclusive cell range). Multi-line uses `\n`.
    pub fn text_range(&self, start_col: u16, start_row: u16, end_col: u16, end_row: u16) -> String {
        let (r1, c1, r2, c2) = if (start_row, start_col) <= (end_row, end_col) {
            (start_row, start_col, end_row, end_col)
        } else {
            (end_row, end_col, start_row, start_col)
        };
        let mut out = String::new();
        for row in r1..=r2 {
            if row as usize >= self.rows as usize {
                break;
            }
            let col_start = if row == r1 { c1 } else { 0 };
            let col_end = if row == r2 {
                c2.min(self.cols.saturating_sub(1))
            } else {
                self.cols.saturating_sub(1)
            };
            let mut line = String::new();
            for col in col_start..=col_end {
                let idx = row as usize * self.cols as usize + col as usize;
                if let Some(cell) = self.cells.get(idx) {
                    line.push(cell.ch);
                }
            }
            out.push_str(line.trim_end());
            if row != r2 {
                out.push('\n');
            }
        }
        out
    }
}

#[derive(Debug)]
pub struct TerminalGrid {
    cols: u16,
    rows: u16,
    cursor_col: u16,
    cursor_row: u16,
    /// Scrollback + visible. Oldest at front.
    lines: Vec<Vec<Cell>>,
    scrollback_limit: usize,
    fg: Color,
    bg: Color,
    attr: CellAttr,
    /// Saved cursor for DECSC/DECRC
    saved_col: u16,
    saved_row: u16,
    title: String,
}

impl TerminalGrid {
    pub fn new(cols: u16, rows: u16, scrollback_limit: usize) -> Self {
        let cols = cols.max(1);
        let rows = rows.max(1);
        let mut lines = Vec::with_capacity(rows as usize);
        for _ in 0..rows {
            lines.push(vec![Cell::default(); cols as usize]);
        }
        Self {
            cols,
            rows,
            cursor_col: 0,
            cursor_row: 0,
            lines,
            scrollback_limit,
            fg: Color::Named(NamedColor::Foreground),
            bg: Color::Named(NamedColor::Background),
            attr: CellAttr::default(),
            saved_col: 0,
            saved_row: 0,
            title: String::new(),
        }
    }

    pub fn cols(&self) -> u16 {
        self.cols
    }

    pub fn rows(&self) -> u16 {
        self.rows
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        let cols = cols.max(1);
        let rows = rows.max(1);
        self.cols = cols;
        self.rows = rows;
        // Keep scrollback; rebuild visible area size by ensuring last `rows` lines exist
        // and each line has `cols` cells.
        for line in &mut self.lines {
            line.resize(cols as usize, Cell::default());
        }
        let visible_start = self.lines.len().saturating_sub(rows as usize);
        // Ensure we have at least `rows` lines
        while self.lines.len() - visible_start < rows as usize {
            self.lines.push(vec![Cell::default(); cols as usize]);
        }
        // Trim excess visible by not storing more than scrollback + rows
        let max_lines = self.scrollback_limit.saturating_add(rows as usize);
        if self.lines.len() > max_lines {
            let drain = self.lines.len() - max_lines;
            self.lines.drain(0..drain);
        }
        self.cursor_col = self.cursor_col.min(cols.saturating_sub(1));
        self.cursor_row = self.cursor_row.min(rows.saturating_sub(1));
    }

    fn visible_start(&self) -> usize {
        self.lines.len().saturating_sub(self.rows as usize)
    }

    fn cell_mut(&mut self, row: u16, col: u16) -> &mut Cell {
        let idx = self.visible_start() + row as usize;
        &mut self.lines[idx][col as usize]
    }

    pub fn put_char(&mut self, c: char) {
        if c == '\n' {
            self.line_feed();
            return;
        }
        if c == '\r' {
            self.cursor_col = 0;
            return;
        }
        if c == '\t' {
            let next = ((self.cursor_col / 8) + 1) * 8;
            self.cursor_col = next.min(self.cols);
            if self.cursor_col >= self.cols {
                self.cursor_col = 0;
                self.line_feed();
            }
            return;
        }
        if c == '\x08' {
            // backspace
            if self.cursor_col > 0 {
                self.cursor_col -= 1;
            }
            return;
        }
        if c == '\x07' {
            // BEL — handled by performer
            return;
        }

        if self.cursor_col >= self.cols {
            self.cursor_col = 0;
            self.line_feed();
        }

        let fg = self.fg;
        let bg = self.bg;
        let attr = self.attr;
        let row = self.cursor_row;
        let col = self.cursor_col;
        let cell = self.cell_mut(row, col);
        cell.ch = c;
        cell.fg = fg;
        cell.bg = bg;
        cell.attr = attr;
        self.cursor_col += 1;
    }

    pub fn line_feed(&mut self) {
        if self.cursor_row + 1 < self.rows {
            self.cursor_row += 1;
            return;
        }
        // Scroll: push a new blank line; drop from scrollback if needed
        self.lines.push(vec![Cell::default(); self.cols as usize]);
        let max_lines = self.scrollback_limit.saturating_add(self.rows as usize);
        if self.lines.len() > max_lines {
            let drain = self.lines.len() - max_lines;
            self.lines.drain(0..drain);
        }
    }

    pub fn carriage_return(&mut self) {
        self.cursor_col = 0;
    }

    pub fn backspace(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        }
    }

    pub fn cursor_up(&mut self, n: u16) {
        self.cursor_row = self.cursor_row.saturating_sub(n.max(1));
    }

    pub fn cursor_down(&mut self, n: u16) {
        let n = n.max(1);
        self.cursor_row = (self.cursor_row + n).min(self.rows.saturating_sub(1));
    }

    pub fn cursor_forward(&mut self, n: u16) {
        let n = n.max(1);
        self.cursor_col = (self.cursor_col + n).min(self.cols.saturating_sub(1));
    }

    pub fn cursor_back(&mut self, n: u16) {
        self.cursor_col = self.cursor_col.saturating_sub(n.max(1));
    }

    pub fn cursor_goto(&mut self, row: u16, col: u16) {
        self.cursor_row = row.saturating_sub(1).min(self.rows.saturating_sub(1));
        self.cursor_col = col.saturating_sub(1).min(self.cols.saturating_sub(1));
    }

    pub fn erase_in_display(&mut self, mode: u16) {
        match mode {
            0 => {
                // cursor to end
                for col in self.cursor_col..self.cols {
                    *self.cell_mut(self.cursor_row, col) = Cell::default();
                }
                for row in (self.cursor_row + 1)..self.rows {
                    for col in 0..self.cols {
                        *self.cell_mut(row, col) = Cell::default();
                    }
                }
            }
            1 => {
                for row in 0..self.cursor_row {
                    for col in 0..self.cols {
                        *self.cell_mut(row, col) = Cell::default();
                    }
                }
                for col in 0..=self.cursor_col {
                    *self.cell_mut(self.cursor_row, col) = Cell::default();
                }
            }
            2 | 3 => {
                let start = self.visible_start();
                for line in &mut self.lines[start..] {
                    for cell in line.iter_mut() {
                        *cell = Cell::default();
                    }
                }
            }
            _ => {}
        }
    }

    pub fn erase_in_line(&mut self, mode: u16) {
        match mode {
            0 => {
                for col in self.cursor_col..self.cols {
                    *self.cell_mut(self.cursor_row, col) = Cell::default();
                }
            }
            1 => {
                for col in 0..=self.cursor_col.min(self.cols.saturating_sub(1)) {
                    *self.cell_mut(self.cursor_row, col) = Cell::default();
                }
            }
            2 => {
                for col in 0..self.cols {
                    *self.cell_mut(self.cursor_row, col) = Cell::default();
                }
            }
            _ => {}
        }
    }

    pub fn sgr(&mut self, params: &[i64]) {
        if params.is_empty() {
            self.reset_sgr();
            return;
        }
        let mut i = 0;
        while i < params.len() {
            match params[i] {
                0 => self.reset_sgr(),
                1 => self.attr.bold = true,
                4 => self.attr.underline = true,
                7 => self.attr.inverse = true,
                22 => self.attr.bold = false,
                24 => self.attr.underline = false,
                27 => self.attr.inverse = false,
                30..=37 => {
                    self.fg = Color::Named(named_from_index((params[i] - 30) as u8, false));
                }
                39 => self.fg = Color::Named(NamedColor::Foreground),
                40..=47 => {
                    self.bg = Color::Named(named_from_index((params[i] - 40) as u8, false));
                }
                49 => self.bg = Color::Named(NamedColor::Background),
                90..=97 => {
                    self.fg = Color::Named(named_from_index((params[i] - 90) as u8, true));
                }
                100..=107 => {
                    self.bg = Color::Named(named_from_index((params[i] - 100) as u8, true));
                }
                38 => {
                    if let Some(c) = parse_extended_color(params, &mut i) {
                        self.fg = c;
                    }
                }
                48 => {
                    if let Some(c) = parse_extended_color(params, &mut i) {
                        self.bg = c;
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }

    fn reset_sgr(&mut self) {
        self.fg = Color::Named(NamedColor::Foreground);
        self.bg = Color::Named(NamedColor::Background);
        self.attr = CellAttr::default();
    }

    pub fn save_cursor(&mut self) {
        self.saved_col = self.cursor_col;
        self.saved_row = self.cursor_row;
    }

    pub fn restore_cursor(&mut self) {
        self.cursor_col = self.saved_col.min(self.cols.saturating_sub(1));
        self.cursor_row = self.saved_row.min(self.rows.saturating_sub(1));
    }

    pub fn snapshot(&self) -> GridSnapshot {
        let start = self.visible_start();
        let mut cells = Vec::with_capacity((self.cols * self.rows) as usize);
        for row in 0..self.rows as usize {
            let line = &self.lines[start + row];
            for col in 0..self.cols as usize {
                cells.push(line.get(col).copied().unwrap_or_default());
            }
        }
        GridSnapshot {
            cols: self.cols,
            rows: self.rows,
            cursor_col: self.cursor_col.min(self.cols.saturating_sub(1)),
            cursor_row: self.cursor_row.min(self.rows.saturating_sub(1)),
            cells,
        }
    }

    /// Visible line strings (for triggers / AI context).
    pub fn visible_lines(&self) -> Vec<String> {
        let start = self.visible_start();
        self.lines[start..]
            .iter()
            .map(|line| {
                let s: String = line.iter().map(|c| c.ch).collect();
                s.trim_end().to_string()
            })
            .collect()
    }
}

fn named_from_index(idx: u8, bright: bool) -> NamedColor {
    match (idx, bright) {
        (0, false) => NamedColor::Black,
        (1, false) => NamedColor::Red,
        (2, false) => NamedColor::Green,
        (3, false) => NamedColor::Yellow,
        (4, false) => NamedColor::Blue,
        (5, false) => NamedColor::Magenta,
        (6, false) => NamedColor::Cyan,
        (7, false) => NamedColor::White,
        (0, true) => NamedColor::BrightBlack,
        (1, true) => NamedColor::BrightRed,
        (2, true) => NamedColor::BrightGreen,
        (3, true) => NamedColor::BrightYellow,
        (4, true) => NamedColor::BrightBlue,
        (5, true) => NamedColor::BrightMagenta,
        (6, true) => NamedColor::BrightCyan,
        (7, true) => NamedColor::BrightWhite,
        _ => NamedColor::Foreground,
    }
}

fn parse_extended_color(params: &[i64], i: &mut usize) -> Option<Color> {
    if *i + 1 >= params.len() {
        return None;
    }
    match params[*i + 1] {
        5 => {
            if *i + 2 >= params.len() {
                return None;
            }
            let idx = params[*i + 2].clamp(0, 255) as u8;
            *i += 2;
            Some(Color::Indexed(idx))
        }
        2 => {
            if *i + 4 >= params.len() {
                return None;
            }
            let r = params[*i + 2].clamp(0, 255) as u8;
            let g = params[*i + 3].clamp(0, 255) as u8;
            let b = params[*i + 4].clamp(0, 255) as u8;
            *i += 4;
            Some(Color::Rgb(r, g, b))
        }
        _ => None,
    }
}
