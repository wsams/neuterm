//! Terminal colors (indexed + RGB).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
    Foreground,
    Background,
    Cursor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Named(NamedColor),
    Indexed(u8),
    Rgb(u8, u8, u8),
}

impl Default for Color {
    fn default() -> Self {
        Self::Named(NamedColor::Foreground)
    }
}

impl Color {
    pub fn to_rgb(self, ansi: &[[u8; 3]; 16], fg: [u8; 3], bg: [u8; 3]) -> [u8; 3] {
        match self {
            Color::Named(NamedColor::Foreground) => fg,
            Color::Named(NamedColor::Background) => bg,
            Color::Named(NamedColor::Cursor) => fg,
            Color::Named(n) => {
                let idx = match n {
                    NamedColor::Black => 0,
                    NamedColor::Red => 1,
                    NamedColor::Green => 2,
                    NamedColor::Yellow => 3,
                    NamedColor::Blue => 4,
                    NamedColor::Magenta => 5,
                    NamedColor::Cyan => 6,
                    NamedColor::White => 7,
                    NamedColor::BrightBlack => 8,
                    NamedColor::BrightRed => 9,
                    NamedColor::BrightGreen => 10,
                    NamedColor::BrightYellow => 11,
                    NamedColor::BrightBlue => 12,
                    NamedColor::BrightMagenta => 13,
                    NamedColor::BrightCyan => 14,
                    NamedColor::BrightWhite => 15,
                    NamedColor::Foreground | NamedColor::Background | NamedColor::Cursor => 7,
                };
                ansi[idx]
            }
            Color::Indexed(i) => {
                if (i as usize) < 16 {
                    ansi[i as usize]
                } else if i < 232 {
                    // 6x6x6 color cube
                    let i = i - 16;
                    let r = i / 36;
                    let g = (i % 36) / 6;
                    let b = i % 6;
                    let level = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
                    [level(r), level(g), level(b)]
                } else {
                    let v = 8 + 10 * (i - 232);
                    [v, v, v]
                }
            }
            Color::Rgb(r, g, b) => [r, g, b],
        }
    }
}
