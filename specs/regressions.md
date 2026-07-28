# Regression Checklist

Agents: run through relevant items before merging behavior changes. Expand this list when bugs escape.

## Tabs

- [ ] New tab via **+** and Super+T both create a working shell
- [ ] Close tab does not crash when one tab remains (closes window or leaves empty — documented behavior)
- [ ] Drag reorder and Super+Shift+Arrows produce the same order model
- [ ] Super+Left/Right switches tabs in visual order
- [ ] Pinned title is not overwritten by process title updates
- [ ] Clearing rename restores process title

## Panes

- [ ] Horizontal and vertical splits create independent shells
- [ ] Nested splits work (≥ 3 levels)
- [ ] Directional focus matches geometry (not creation order)
- [ ] Closing a pane reflows layout without leaving gaps/ghost panes
- [ ] Divider drag updates ratio and survives redraw

## Appearance / config

- [ ] Font family/size change applies without destroying cols/rows unexpectedly
- [ ] Background color applies with and without image
- [ ] Overlay opacity darkens/lightens image as expected
- [ ] Blur does not drop frame rate to unusable on typical images (cached)
- [ ] Invalid YAML reload keeps last-good config

## Performance

- [ ] Large scrollback config does not freeze UI on open
- [ ] Flood of output remains scrollable/interruptible (Ctrl+C reaches PTY)
- [ ] AI/chat network stall does not block typing in terminal panes

## Triggers

- [ ] `production` line highlight rule styles whole line
- [ ] Bad regex in config skips rule without crashing
- [ ] Disabling `triggers.enabled` removes styles immediately on reload

## AI

- [ ] With `ai.enabled: false`, shortcuts no-op or show “enable in config”
- [ ] Command help never auto-executes
- [ ] Esc dismisses command help without changing the prompt line
- [ ] Chat pane works when Ollama is down (shows error, terminal still works)

## Platform

- [ ] Linux Wayland and X11 both start (where available)
- [ ] Windows ConPTY starts default shell
- [ ] macOS Cmd-based defaults match keybindings spec
