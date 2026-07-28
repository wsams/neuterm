# Configuration (YAML)

## Location

| Platform | Path |
|----------|------|
| Linux | `$XDG_CONFIG_HOME/neuterm/config.yaml` (fallback `~/.config/neuterm/config.yaml`) |
| macOS | `~/Library/Application Support/neuterm/config.yaml` **or** `~/.config/neuterm/config.yaml` (both accepted; XDG preferred if present) |
| Windows | `%APPDATA%\neuterm\config.yaml` |

On first run, NeuTerm creates the directory and copies `configs/default.yaml` if no user config exists.

Override with env: `NEUTERM_CONFIG=/path/to/config.yaml`.

## Format

- YAML 1.1/1.2 via `serde_yaml`
- Unknown keys: **warn and ignore** (forward compatible)
- Invalid values: fail that key with a clear error; keep last-good config if reload fails
- Hot reload: watch the config file; apply safe changes live (colors, font size, keybindings, triggers, AI endpoints). Changes requiring restart (e.g. GPU backend force) are documented and logged.

## Schema (v1)

```yaml
# NeuTerm configuration — see specs/configuration.md

window:
  initial_cols: 120
  initial_rows: 40
  # padding in pixels
  padding:
    left: 8
    right: 8
    top: 8
    bottom: 8

font:
  family: "JetBrains Mono"
  size: 14.0
  # optional: weight, ligatures
  ligatures: false

colors:
  foreground: "#a5a7aa"
  background: "#25282e"
  cursor: "#a5a6aa"
  selection_fg: "#000000"
  selection_bg: "#7d7d7d"
  ansi:
    - "#2e3239"  # black
    - "#be861b"  # red
    - "#2289b4"  # green
    - "#d1b06e"  # yellow
    - "#7d8fa4"  # blue
    - "#a25795"  # magenta
    - "#5abfd5"  # cyan
    - "#a5a6aa"  # white
  brights:
    - "#2e3239"
    - "#bd851b"
    - "#2289b4"
    - "#d0af6e"
    - "#7d8fa4"
    - "#a25794"
    - "#4ea7bb"
    - "#a5a6aa"
  split: "#a5a7aa"

background:
  # solid color always available via colors.background
  image: null            # path string, or null
  # overlay color drawn over image (darken/lighten)
  overlay_color: "#25282e"
  overlay_opacity: 0.90  # 0.0–1.0; higher = more color, less image
  # filter applied to the image layer
  filter:
    brightness: 1.0      # 0.0–2.0
    blur: 0.0            # px, 0 = off
    saturation: 1.0

scrollback:
  # integer lines, or "unlimited"
  lines: 200000

bell:
  # visual | audible | both | pulse | none
  mode: visual
  # pulse = soft glow on active pane (preferred "less jarring" option)
  # visual = brief screen/pane flash
  audible: false

term:
  # TERM value for child processes
  program: "xterm-256color"
  shell: null            # null = platform default login shell
  cwd: null              # null = cwd of NeuTerm process

tabs:
  show_bar: true
  hide_if_only_one: false
  # auto = process title; pinned custom title wins when set
  default_title_mode: process

panes:
  inactive_dim: 0.15     # darken inactive panes slightly (0–1)

keybindings:
  # See specs/keybindings.md — map action -> chord
  # Platform modifier: Super on macOS (Cmd), Super/Win on Linux/Windows
  # Users may override any binding here.
  bindings: []           # empty = use built-in defaults

triggers:
  # See specs/plugins-triggers.md
  enabled: true
  rules: []

plugins:
  enabled: true
  # directories searched for plugin manifests / code
  dirs: []

ai:
  # See specs/ai-agents.md
  enabled: false
  ollama:
    host: "127.0.0.1"
    port: 11434
    # full base URL override; if set, host/port ignored
    base_url: null
    model: "llama3.2"
    timeout_ms: 60000
  command_help:
    shortcut: "default"  # use built-in Super+;
    # system prompt override (optional)
    system_prompt: null
  chat:
    # open chat pane action uses default keybinding
    title: "AI Chat"

performance:
  # auto | gpu | cpu
  renderer: auto
  vsync: true
```

## Validation rules

- `initial_cols` / `initial_rows`: ≥ 1 (recommended ≥ 20 / ≥ 5)
- Color strings: `#RRGGBB` or `#RRGGBBAA`
- `scrollback.lines`: positive integer or string `"unlimited"`
- `background.filter.blur`: ≥ 0
- Ollama port: 1–65535

## Example: production-line trigger

```yaml
triggers:
  enabled: true
  rules:
    - name: production-highlight
      match: "(?i)(prod|production|prd|critical)"
      match_type: regex   # regex | string
      scope: line         # line | match
      style:
        foreground: "#ffffff"
        background: "#c0392b"
```

## Reloading

- File watcher on config path
- On change: parse → validate → swap `Arc<Config>`
- Failures: keep previous config, surface toast/log error
