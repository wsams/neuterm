# Appearance

## Font

- Configurable family and size (`font.family`, `font.size`).
- Ligatures off by default (matches user's WezTerm preference).
- Fallback fonts: system monospace chain if primary missing; log a warning.
- Changing font must not forcibly resize the window to preserve row/col (WezTerm: `adjust_window_size_when_changing_font_size = false`).

## Colors

- Full 16-color ANSI + brights, foreground, background, cursor, selection, split.
- Background color applies when no image, and as overlay when image is set.

## Background image

- Optional path: `background.image`.
- Cover/center scaling (image covers the terminal content area).
- Overlay: `overlay_color` + `overlay_opacity` to darken/lighten (WezTerm-style color layer over image).
- Filters:
  - `brightness` — scale image luminance
  - `blur` — Gaussian blur in px (0 = off); applied to image layer only, not text
  - `saturation` — optional

Performance: heavy blur is expensive — apply on image load / resize, cache texture; do not re-blur every frame.

## Initial window size

- `window.initial_cols` / `window.initial_rows` set the character grid when a **new window** opens.
- Pixel size = cols × cell_width + padding (and chrome).

## Bell

Modes (`bell.mode`):

| Mode | Behavior |
|------|----------|
| `none` | No feedback |
| `visual` | Brief flash of the pane/window |
| `audible` | System/beep sound |
| `both` | visual + audible |
| `pulse` | Soft glow pulse on the active pane border/content (default recommendation for "obvious but not jarring") |

Default: `visual` or `pulse` with `audible: false` (user's WezTerm: audible disabled, visual enabled). **Ship default: `pulse`.**

## Cursor

- Steady block by default; blink optional later.
- Respect DEC cursor style sequences from apps when reasonable.

## Padding

- Per-side window padding in pixels; content does not draw under the tab bar.
