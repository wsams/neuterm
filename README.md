# NeuTerm

Fast, configurable terminal emulator for Linux, macOS, and Windows.

Project specifications for contributors and agents live in [`specs/SPECS.md`](specs/SPECS.md).

## Quick start

### Prerequisites

- Rust stable ([rustup](https://rustup.rs/))
- Platform libraries — see [`specs/build-and-run.md`](specs/build-and-run.md)

### Build & run

```bash
cargo build --release
cargo run --release
```

Debug build:

```bash
cargo run
```

### Config

On first launch NeuTerm writes a default config to:

- Linux: `~/.config/neuterm/config.yaml`
- macOS: `~/Library/Application Support/neuterm/config.yaml` (or `~/.config/neuterm/config.yaml`)
- Windows: `%APPDATA%\neuterm\config.yaml`

Override with `NEUTERM_CONFIG=/path/to/config.yaml` or `neuterm --config path`.

Shipped defaults: [`configs/default.yaml`](configs/default.yaml).

### Useful env vars

| Variable | Purpose |
|----------|---------|
| `NEUTERM_CONFIG` | Config path |
| `RUST_LOG` | e.g. `neuterm=debug` |

### Tests

```bash
cargo test --workspace
```

## Features (MVP → roadmap)

Already sketched in code + specs:

- YAML settings, tabs, splits, triggers, visual/pulse bell
- Ollama config, command-help overlay, AI chat pane stub
- CPU renderer now; GPU path planned (`performance.renderer`)

See the [spec index](specs/SPECS.md) for architecture decisions and regression checks.
