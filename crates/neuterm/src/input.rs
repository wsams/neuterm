//! Keybinding dispatch.
//!
//! See `specs/keybindings.md`.

use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, ModifiersState, NamedKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    TabPrev,
    TabNext,
    TabMoveLeft,
    TabMoveRight,
    TabNew,
    TabClose,
    TabRename,
    PaneSplitHorizontal,
    PaneSplitVertical,
    PaneFocusLeft,
    PaneFocusRight,
    PaneFocusUp,
    PaneFocusDown,
    AiCommandHelp,
    AiChatToggle,
    ReloadConfig,
    Copy,
    Paste,
}

pub fn action_from_key(event: &KeyEvent, mods: ModifiersState) -> Option<Action> {
    if event.state != ElementState::Pressed || event.repeat {
        return None;
    }

    let super_key = mods.super_key();
    let shift = mods.shift_key();
    let alt = mods.alt_key();
    let ctrl = mods.control_key();

    // Terminal copy/paste: Ctrl+Shift+C/V (Linux) and Super+C/V
    if let Key::Character(c) = &event.logical_key {
        let c = c.as_str();
        if (ctrl && shift && c.eq_ignore_ascii_case("c"))
            || (super_key && !shift && c.eq_ignore_ascii_case("c"))
        {
            return Some(Action::Copy);
        }
        if (ctrl && shift && c.eq_ignore_ascii_case("v"))
            || (super_key && !shift && c.eq_ignore_ascii_case("v"))
        {
            return Some(Action::Paste);
        }
    }

    // Super+arrow tab navigation / move
    if super_key {
        match &event.logical_key {
            Key::Named(NamedKey::ArrowLeft) if shift && !alt => return Some(Action::TabMoveLeft),
            Key::Named(NamedKey::ArrowRight) if shift && !alt => return Some(Action::TabMoveRight),
            Key::Named(NamedKey::ArrowLeft) if alt && !shift => return Some(Action::PaneFocusLeft),
            Key::Named(NamedKey::ArrowRight) if alt && !shift => return Some(Action::PaneFocusRight),
            Key::Named(NamedKey::ArrowUp) if alt => return Some(Action::PaneFocusUp),
            Key::Named(NamedKey::ArrowDown) if alt => return Some(Action::PaneFocusDown),
            Key::Named(NamedKey::ArrowLeft) if !shift && !alt => return Some(Action::TabPrev),
            Key::Named(NamedKey::ArrowRight) if !shift && !alt => return Some(Action::TabNext),
            Key::Character(c) => {
                let c = c.as_str();
                match (c, shift, alt) {
                    ("t", false, false) => return Some(Action::TabNew),
                    ("w", false, false) => return Some(Action::TabClose),
                    ("w", true, false) => return Some(Action::TabClose),
                    ("i", false, false) => return Some(Action::TabRename),
                    ("d", false, false) => return Some(Action::PaneSplitHorizontal),
                    ("d", true, false) => return Some(Action::PaneSplitVertical),
                    (";", false, false) => return Some(Action::AiCommandHelp),
                    ("/", false, false) => return Some(Action::AiCommandHelp),
                    ("a", true, false) => return Some(Action::AiChatToggle),
                    ("r", true, false) => return Some(Action::ReloadConfig),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    None
}

/// Map a key event to bytes for the PTY (when not handled as an action).
pub fn key_to_pty_bytes(event: &KeyEvent, mods: ModifiersState) -> Option<Vec<u8>> {
    if event.state != ElementState::Pressed {
        return None;
    }

    // Don't send Super chords or Ctrl+Shift (copy/paste) to PTY
    if mods.super_key() {
        return None;
    }
    if mods.control_key() && mods.shift_key() {
        return None;
    }

    match &event.logical_key {
        Key::Named(NamedKey::Enter) => Some(b"\r".to_vec()),
        Key::Named(NamedKey::Backspace) => Some(b"\x7f".to_vec()),
        Key::Named(NamedKey::Tab) => Some(b"\t".to_vec()),
        Key::Named(NamedKey::Escape) => Some(b"\x1b".to_vec()),
        Key::Named(NamedKey::ArrowUp) => Some(b"\x1b[A".to_vec()),
        Key::Named(NamedKey::ArrowDown) => Some(b"\x1b[B".to_vec()),
        Key::Named(NamedKey::ArrowRight) => Some(b"\x1b[C".to_vec()),
        Key::Named(NamedKey::ArrowLeft) => Some(b"\x1b[D".to_vec()),
        Key::Named(NamedKey::Home) => Some(b"\x1b[H".to_vec()),
        Key::Named(NamedKey::End) => Some(b"\x1b[F".to_vec()),
        Key::Named(NamedKey::PageUp) => Some(b"\x1b[5~".to_vec()),
        Key::Named(NamedKey::PageDown) => Some(b"\x1b[6~".to_vec()),
        Key::Named(NamedKey::Delete) => Some(b"\x1b[3~".to_vec()),
        Key::Character(c) => {
            if mods.control_key() {
                let ch = c.chars().next()?;
                let lower = ch.to_ascii_lowercase();
                if ('a'..='z').contains(&lower) {
                    return Some(vec![(lower as u8) - b'a' + 1]);
                }
            }
            Some(c.as_str().as_bytes().to_vec())
        }
        _ => None,
    }
}
