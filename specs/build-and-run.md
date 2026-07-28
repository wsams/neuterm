# Build & Run

## Prerequisites

- Rust toolchain (stable) via [rustup](https://rustup.rs/)
- Platform dependencies:

### Linux

```bash
# Fedora (traditional)
sudo dnf install -y gcc make pkg-config fontconfig-devel freetype-devel \
  libxkbcommon-devel wayland-devel libxcb-devel

# Debian / Ubuntu
sudo apt install -y build-essential pkg-config libfontconfig1-dev \
  libfreetype6-dev libxkbcommon-dev libwayland-dev \
  libxcb-shape0-dev libxcb-xfixes0-dev
```

**Fedora Atomic / Bazzite:** `dnf install` is blocked (opens docs). For a temporary writable `/usr` overlay (discarded on reboot):

```bash
sudo rpm-ostree usroverlay
sudo dnf5 install -y fontconfig-devel freetype-devel libxkbcommon-devel \
  wayland-devel libxcb-devel
```

Or layer packages permanently with `rpm-ostree install …` (requires reboot). For longer-term dev, prefer a Distrobox/Toolbox container with a normal userspace toolchain.

### macOS

- Xcode Command Line Tools: `xcode-select --install`
- Optional: Homebrew for fonts

### Windows

- Visual Studio Build Tools with C++ workload
- Windows 10/11 SDK

## Build

From repository root:

```bash
cargo build --release
```

Debug (faster compile):

```bash
cargo build
```

Binary path:

- Debug: `target/debug/neuterm`
- Release: `target/release/neuterm`

## Run

```bash
cargo run
# or
cargo run --release
# or
./target/release/neuterm
```

Useful env vars:

| Variable | Purpose |
|----------|---------|
| `NEUTERM_CONFIG` | Override config path |
| `RUST_LOG` | e.g. `neuterm=debug` |
| `NEUTERM_PERF=1` | Frame / backlog metrics |

## Test

```bash
cargo test --workspace
```

## Install (dev)

```bash
cargo install --path crates/neuterm
```

## Packaging (later)

- Linux: `.tar.gz`, optional Flatpak / `.deb` / `.rpm`
- macOS: `.app` + notarized `.dmg`
- Windows: `.msi` / portable `.exe`

Until packaging lands, `cargo install` / release binaries are the install path.

## Agent checklist

Before claiming “it builds”:

1. `cargo build` succeeds on the agent’s platform
2. `cargo test` passes
3. `cargo run` opens a window with a shell (when display available)
