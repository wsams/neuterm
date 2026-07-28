# NeuTerm — Project Specifications

**NeuTerm** is a cross-platform terminal emulator optimized for speed by default, with deep configurability and optional AI agent integration.

This document is the entry point for agents and contributors. Detailed requirements live in linked files. When behavior changes, update the relevant spec **in the same change**.

---

## Goals

1. **Fast by default** — Minimal overhead out of the box; large scrollback and GPU-accelerated rendering where available.
2. **Simple to install** — First-class packages / binaries for Linux, macOS, and Windows.
3. **Configurable** — A single YAML settings file drives appearance, keybindings, panes, plugins, triggers, and AI.
4. **Moddable** — Plugins and triggers can extend behavior; heavy mods may trade speed for power (user's choice).
5. **AI-ready** — Native Ollama integration for a chat pane and non-intrusive command help.

Non-goals for v1: full WezTerm/iTerm feature parity, multiplexed remote domains, SSH client built-in, marketplace for plugins.

---

## Spec index

| Spec | File | Summary |
|------|------|---------|
| Architecture & language | [architecture.md](./architecture.md) | Stack choice (Rust), crates, process model, performance principles |
| Configuration | [configuration.md](./configuration.md) | YAML settings schema, paths, reload behavior |
| Tabs & window chrome | [tabs.md](./tabs.md) | Tabs, naming, process titles, reorder/close/new, shortcuts |
| Panes & splits | [panes.md](./panes.md) | Horizontal/vertical splits, navigation, focus |
| Appearance | [appearance.md](./appearance.md) | Font, colors, background image, filters, bell, initial size |
| Theming | [theming.md](./theming.md) | Swappable Theme object (colors + tab/pane chrome) |
| Plugins & triggers | [plugins-triggers.md](./plugins-triggers.md) | Plugin injection model, regex/string triggers & transforms |
| Performance | [performance.md](./performance.md) | Scrollback, GPU path, latency budgets |
| AI / Ollama | [ai-agents.md](./ai-agents.md) | Ollama host/model config, chat pane, command-help overlay |
| Build & run | [build-and-run.md](./build-and-run.md) | How to build, run, test, and package |
| Cross-platform | [cross-platform.md](./cross-platform.md) | Linux / macOS / Windows install & platform quirks |
| Keybindings | [keybindings.md](./keybindings.md) | Default shortcuts (tabs, panes, AI, bell) |
| Regression checklist | [regressions.md](./regressions.md) | Behaviors that must not regress |

---

## Product principles

- **Speed first.** Features that add measurable latency must be opt-in or off the hot path (render/PTY read).
- **YAML is the source of truth** for user settings (not a GUI-only store).
- **Defaults should feel complete** without plugins; plugins/triggers/AI are additive.
- **Inspiration, not clones.** UX cues from WezTerm/iTerm (tabs, splits, triggers, visual bell) are welcome; APIs need not match.
- **Document decisions here.** Architecture trade-offs belong in these specs so future agents do not re-litigate them.

---

## Current milestone status

| Area | Status |
|------|--------|
| Specs authored | ✅ |
| Rust workspace scaffold | ✅ |
| YAML config load | ✅ |
| Window + PTY + basic CPU render | ✅ monospace grid via fontdue |
| Tabs / splits (model + shortcuts) | ✅ MVP (drag-reorder polish later) |
| Selection + copy/paste | ✅ MVP (drag select, Ctrl+Shift+C/V) |
| CSI DA/DSR (fish compatibility) | ✅ |
| Triggers | ✅ MVP (line/match styling) |
| Plugins host | 🚧 stub (config + dirs only) |
| GPU path (`wgpu`) | planned |
| Background image + blur | 🚧 config wired; image decode/composite next |
| Ollama config + command help + chat pane | ✅ MVP (chat UI minimal; streaming client ready) |

Update this table as milestones land.

---

## Working agreements for agents

1. Read this file and the relevant linked specs before implementing a feature.
2. Prefer established crates over custom implementations (VT parse, PTY, fonts, GPU).
3. Keep the hot path (PTY → grid → GPU) free of blocking I/O and plugin/AI work.
4. When adding a setting, update [configuration.md](./configuration.md) and the example config in the same PR/change.
5. When changing UX that users rely on (tabs, splits, keybindings), update [regressions.md](./regressions.md).
6. Do not invent branding/marketing copy; keep docs technical.
