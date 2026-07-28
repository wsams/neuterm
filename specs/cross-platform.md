# Cross-Platform Install & Support

## Supported platforms (v1 targets)

| OS | Arch | Status |
|----|------|--------|
| Linux | x86_64, aarch64 | Primary development |
| macOS | Apple Silicon + Intel | Supported |
| Windows | x86_64 | Supported (PTY via ConPTY) |

## Install goals (“easy to install”)

1. **Prebuilt binaries** per platform on GitHub Releases.
2. **Package managers** (stretch): Homebrew (macOS/Linux), Scoop/winget (Windows), optional distro packages.
3. **`cargo install neuterm`** once published to crates.io.

Document exact install commands in the root README as they become available.

## Platform notes

### Linux

- Prefer Wayland; X11 via `winit` fallback.
- Shell: `$SHELL` or `/bin/bash`.
- Config: XDG.

### macOS

- Super key = Cmd for defaults matching user’s WezTerm/iTerm muscle memory.
- Option/Alt: prefer “Normal” composed keys unless apps request meta (configurable later).
- App bundle for Dock; terminal entitlements as needed.

### Windows

- ConPTY for PTY.
- Default shell: PowerShell or `ComSpec`; allow `term.shell` override (e.g. `pwsh`, WSL launcher later).
- Paths in config accept Windows paths for background images; YAML forward slashes OK.

## CI matrix (planned)

- `ubuntu-latest`, `macos-latest`, `windows-latest` — build + unit tests.
- Integration/UI tests gated where no display is available.
