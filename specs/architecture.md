# Architecture & Language

## Language decision: **Rust**

**Choice:** Rust (edition 2021+).

**Why not Go:**

| Criterion | Rust | Go |
|-----------|------|-----|
| Terminal emulator precedent | Alacritty, WezTerm, Ghostty (Zig), Rio — Rust dominates high-perf terminals | Few mature GPU terminals |
| Native speed / zero-cost abstractions | Excellent for render + VT parse hot paths | GC pauses can hurt large scrollback / high fps |
| GPU / windowing ecosystem | `wgpu`, `winit`, `glyphon`/`cosmic-text` mature | Weaker desktop GPU stack |
| Windows + macOS + Linux | Excellent via `winit`/`wgpu` | Good CLI, weaker native GPU UI |
| Plugin sandboxing options | WASM (e.g. `wasmtime`) or dynamic libs later | Possible but less common in this domain |

**Decision locked:** NeuTerm is a Rust project. Revisit only if a hard blocker appears (unlikely).

---

## High-level architecture

```
┌─────────────────────────────────────────────────────────────┐
│  UI shell (winit window + GPU surface)                      │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────────────┐  │
│  │ Tab bar     │  │ Pane tree    │  │ Overlays (AI help, │  │
│  │             │  │ (splits)     │  │  chat, rename)     │  │
│  └─────────────┘  └──────┬───────┘  └────────────────────┘  │
│                          │                                   │
│  ┌───────────────────────▼────────────────────────────────┐ │
│  │ Renderer (GPU preferred; CPU fallback)                 │ │
│  │  - glyph atlas / text shaping                          │ │
│  │  - background color / image + filters                  │ │
│  │  - trigger highlight pass                              │ │
│  └───────────────────────▲────────────────────────────────┘ │
└──────────────────────────┼──────────────────────────────────┘
                           │ TerminalGrid snapshots
┌──────────────────────────┼──────────────────────────────────┐
│  Session / Mux                                            │
│  ┌────────────┐   ┌──────▼──────┐   ┌───────────────────┐ │
│  │ Tab model  │──▶│ Pane model  │──▶│ PtySession        │ │
│  └────────────┘   └─────────────┘   │  - PTY read/write │ │
│                                      │  - VT parser      │ │
│                                      │  - scrollback     │ │
│                                      └───────────────────┘ │
│  Config (YAML) · Keybindings · Plugins · Triggers · AI    │
└───────────────────────────────────────────────────────────┘
```

### Process model

- **Single process** for v1 (UI + mux + PTY readers on threads/async tasks).
- Each pane owns a PTY and a terminal grid (scrollback + visible).
- PTY I/O runs on a dedicated thread or async runtime (`tokio`); UI thread never blocks on reads.
- Plugins (later): prefer out-of-hot-path execution; WASM sandbox is the target model once the MVP renders.

### Crate layout (workspace)

```
neuterm/
  Cargo.toml              # workspace
  crates/
    neuterm/              # binary: main, window loop
    neuterm-config/       # YAML schema, load/validate
    neuterm-term/         # PTY, VT, grid, scrollback
    neuterm-render/       # GPU/CPU rendering
    neuterm-mux/          # tabs, panes, focus, splits
    neuterm-triggers/     # match + transform pipeline
    neuterm-plugins/      # plugin host (stub → WASM)
    neuterm-ai/           # Ollama client, chat, command help
  configs/
    default.yaml          # shipped defaults
  specs/                  # this folder
```

Crates may be collapsed early (e.g. single binary crate) and split when boundaries stabilize. Specs describe the **logical** modules even if physically merged.

---

## Core dependencies (preferred)

Use well-established crates; avoid reinventing VT/PTY/GPU:

| Concern | Preferred crates |
|---------|------------------|
| Window / input | `winit` |
| GPU | `wgpu` |
| Text shaping / glyphs | `cosmic-text` or `glyphon` |
| PTY | `portable-pty` (or `alacritty_terminal` which bundles PTY+VT) |
| VT / grid | `alacritty_terminal` **or** `vte` + custom grid; prefer reuse |
| Config | `serde` + `serde_yaml` |
| Async | `tokio` |
| CLI | `clap` |
| Logging | `tracing` + `tracing-subscriber` |
| HTTP (Ollama) | `reqwest` |

**Architecture decision:** Prefer `alacritty_terminal` for PTY+VT+grid initially to ship a working emulator quickly, then specialize if needed.

---

## Performance principles

See [performance.md](./performance.md). Summary:

1. Never run AI, plugin scripts, or disk I/O on the render or PTY-parse path.
2. Triggers run as a lightweight pass over lines (compiled regex cache).
3. Scrollback is a ring buffer; configurable size including "unlimited" (practical memory cap with warning).
4. GPU path is default when a capable adapter exists; CPU fallback must still work.

---

## Inspiration

User WezTerm config (`~/.config/wezterm/wezterm.lua`) informs defaults:

- Large scrollback (~200k lines)
- Visual bell (no audible by default)
- Background image + darkening overlay
- Fancy tabs with process title / pinned custom title
- Cmd/Super + arrows for tabs; Super+Shift+arrows to reorder
- Super+D / Super+Shift+D for splits
- Super+Opt+arrows for pane focus
- Initial cols/rows configurable (user uses large defaults)

NeuTerm defaults should feel familiar to that workflow without copying WezTerm APIs.
