# Panes & Splits

## Requirements

1. Split a pane **horizontally** or **vertically** into independent terminal sessions.
2. Splits can nest **any number of times** (binary tree of split nodes / leaf panes).
3. Navigate focus between panes with keyboard (and mouse click).
4. Closing a pane removes it and reflows the tree; closing the last pane in a tab closes the tab.

## Split model

Binary tree:

```
enum Node {
  Leaf(PaneId),
  Split { direction: Horizontal | Vertical, ratio: f32, first: Box<Node>, second: Box<Node> },
}
```

- **SplitHorizontal** (WezTerm naming): side-by-side (vertical divider) — bound to Super+D by default.
- **SplitVertical**: stacked (horizontal divider) — Super+Shift+D.

Each leaf is a full `PtySession` (independent shell).

## Navigation

| Action | Default |
|--------|---------|
| Focus left/right/up/down | Super+Alt+Arrow |
| Split horizontal | Super+D |
| Split vertical | Super+Shift+D |
| Close pane | Super+Shift+W (or close when shell exits — configurable) |
| Zoom pane (optional v1.1) | Super+Shift+Enter |

Mouse: click focuses pane. Divider drag adjusts `ratio`.

## Appearance

- Split divider color from `colors.split`.
- Inactive panes may apply a slight dim (`panes.inactive_dim`) so focus is obvious.
- Bell/pulse targets the **active** pane by default; configurable later for "bell in background pane" indicators.

## AI chat pane

The AI chat UI may appear as:

1. A **special leaf** in the pane tree (preferred for "simple pane that has a chat window"), or
2. A floating overlay.

**v1 decision:** Chat opens as a split leaf with a non-PTY content type (`PaneKind::AiChat`), so it participates in the same focus/navigation model. Command-help is a **modal overlay**, not a pane (non-intrusive, dismissible).

## Inspiration

User WezTerm bindings:

- Super+D / Super+Shift+D splits
- Super+Opt+Arrows for directional focus
- Inactive pane HSB transform (NeuTerm uses simpler dim for v1; advanced tinting later)
