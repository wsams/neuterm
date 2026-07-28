# Keybindings

`Super` = Cmd (macOS) / Super-Win (Linux/Windows). Users can remap everything under `keybindings.bindings` in YAML.

## Tabs

| Action | Default |
|--------|---------|
| `tab_prev` | Super+Left |
| `tab_next` | Super+Right |
| `tab_move_left` | Super+Shift+Left |
| `tab_move_right` | Super+Shift+Right |
| `tab_new` | Super+T |
| `tab_close` | Super+W |
| `tab_rename` | Super+I |

## Panes

| Action | Default |
|--------|---------|
| `pane_split_horizontal` | Super+D |
| `pane_split_vertical` | Super+Shift+D |
| `pane_focus_left` | Super+Alt+Left |
| `pane_focus_right` | Super+Alt+Right |
| `pane_focus_up` | Super+Alt+Up |
| `pane_focus_down` | Super+Alt+Down |
| `pane_close` | Super+Shift+W |

## AI

| Action | Default |
|--------|---------|
| `ai_command_help` | Super+; |
| `ai_chat_toggle` | Super+Shift+A |

## Misc

| Action | Default |
|--------|---------|
| `reload_config` | Super+Shift+R |
| `copy` | Ctrl+Shift+C (Linux/Win) or Super+C |
| `paste` | Ctrl+Shift+V (Linux/Win) or Super+V |

Drag with the left mouse button to select text, then copy.

## YAML override format

```yaml
keybindings:
  bindings:
    - key: LeftArrow
      mods: [Super]
      action: tab_prev
    - key: semicolon
      mods: [Super]
      action: ai_command_help
```

Conflicts: last binding wins; warn on duplicate action bindings.
