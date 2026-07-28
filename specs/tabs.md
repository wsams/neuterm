# Tabs

## Requirements

1. The window supports **multiple tabs**, each containing a pane tree (see [panes.md](./panes.md)).
2. Tabs can be **named** (custom/pinned title).
3. By default, tab titles **auto-update** from the active pane's process / OSC title.
4. Users can **drag** tabs to reorder, **close** tabs, and click **+** to create a new tab.
5. Keyboard shortcuts manage tabs (switch, move, new, close, rename) — see [keybindings.md](./keybindings.md).

## Title resolution

Priority (highest first):

1. **Pinned custom title** — set via rename action / prompt; persists until cleared.
2. **Process / pane title** — from shell OSC 0/2 or active process name.
3. Fallback: `"Shell"` or tab index.

Visual cue for pinned titles: a small pin indicator (similar to WezTerm pin emoji, but keep subtle — e.g. pin glyph or distinct style). Clearing the custom title restores auto mode.

## Tab bar UX

| Affordance | Behavior |
|------------|----------|
| Click tab | Activate |
| Middle-click / close button | Close tab (confirm if process still running — configurable later; v1: confirm if more than shell) |
| Drag tab | Reorder |
| **+** button | New tab (default shell, cwd = active tab cwd when possible) |
| Double-click tab / rename shortcut | Prompt for custom name; empty input clears pin |

`tabs.hide_if_only_one`: when true, hide bar if single tab (default **false** so + remains visible).

## Keyboard

Defaults use arrow keys (user preference):

| Action | Default (macOS / Linux+Win) |
|--------|-----------------------------|
| Previous tab | Super+Left |
| Next tab | Super+Right |
| Move tab left | Super+Shift+Left |
| Move tab right | Super+Shift+Right |
| New tab | Super+T |
| Close tab | Super+W |
| Rename tab | Super+I |

`Super` = Cmd on macOS, Super/Win on Linux/Windows. Ctrl alternatives may be added for Linux users who prefer Ctrl+PageUp-style bindings via config.

## State model

```
Window
 └── TabBar
      └── Tab[] (ordered)
           ├── id, title_override: Option<String>
           └── PaneTree (root split node)
```

Closing the last tab closes the window (with confirmation if configured).

## Regression notes

- Reorder via drag and via shortcuts must stay consistent with the same underlying order.
- Auto titles must not overwrite a pinned title.
- New tab inherits sensible cwd from the previously active pane when available.
