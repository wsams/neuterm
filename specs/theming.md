# Theming

NeuTerm visuals are driven by a single **theme** object. Swap the theme → new look.

## Selecting a theme

```yaml
theme: graphite   # builtin
# theme: midnight
```

Or inline a full object (overrides the builtin of the same `name` base):

```yaml
theme:
  name: custom
  window_padding: { left: 10, right: 10, top: 10, bottom: 10 }
  # Margin outside the pane focus border. Top 0 = tab bar flush on content.
  pane_inset: { left: 4, right: 4, top: 0, bottom: 4 }
  tabs:
    gap: 6
    bar_padding: { left: 6, right: 6, top: 6, bottom: 6 }
    tab_padding: { left: 6, right: 6, top: 6, bottom: 6 }
    bar_background: "#1c1f24"
    active_background: "#2e3239"
    inactive_background: "#1c1f24"
    active_foreground: "#a5a7aa"
    inactive_foreground: "#6a6e76"
    separator_color: "#3a3f48"
    separator_height: 1
  panes:
    inactive_dim: 0.15
    focus_border: "#a5a6aa"
  colors:
    foreground: "#a5a7aa"
    background: "#25282e"
    # ...ansi / brights / cursor / selection / split
```

## Builtins

| Name | Notes |
|------|--------|
| `graphite` | Default; matches the original NeuTerm palette |
| `midnight` | Darker blue-black chrome |

## Chrome metrics

All spacing lives on the theme object (swap theme → new look):

| Field | Role |
|-------|------|
| `window_padding` | Inside the focus border, around the cell grid |
| `pane_inset` | Outside the focus border (sides/bottom; top usually `0`) |
| `tabs.bar_padding` | Even space around the row of tabs inside the bar |
| `tabs.tab_padding` | Even space inside each tab chip and the `+` button |
| `tabs.gap` | Horizontal space between chips |

Derived sizes:

- Tab / `+` height = `cell_height + tab_padding.top + tab_padding.bottom`
- Bar height = `bar_padding.top + tab_height + bar_padding.bottom + separator_height`

## Legacy keys

When `theme` is a **name** (not a full object), top-level `colors`, `window.padding`, and `panes.inactive_dim` still override the builtin. Prefer putting new visual knobs under `theme`.
