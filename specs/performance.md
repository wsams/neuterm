# Performance

## Priority

**Speed over everything else** for the default configuration. Mods (plugins, AI, heavy blur, huge images) may slow things down — that is acceptable when opted into.

## Targets (aspirational v1)

| Metric | Target |
|--------|--------|
| Input latency (key → PTY) | < 5 ms typical |
| Render under load | 60 fps sustained on modern integrated GPU |
| Throughput | Handle noisy `cat` / build logs without UI freeze |
| Startup to prompt | < 200 ms after process start on warm disk (best effort) |

## Scrollback

- Default: large buffer (`200000` lines), inspired by WezTerm unlimited-ish usage.
- Configurable integer or `"unlimited"`.
- Implementation: ring buffer of lines/cells; `"unlimited"` grows until memory pressure — set a soft safety cap (e.g. warn at N GB) and document it.
- Scrolling must not allocate per frame; reuse GPU buffers.

## GPU acceleration

WezTerm/Alacritty-style speed bumps to adopt:

1. **GPU glyph atlas** — cache rasterized glyphs in a texture atlas.
2. **Damage tracking** — redraw only dirty rows/cells when possible.
3. **Background image as texture** — upload once; composite with overlay in shader.
4. **Blur offline** — pre-blur to texture on load/resize.
5. **vsync** configurable; uncapped mode for latency testing.
6. **`renderer: auto|gpu|cpu`** — CPU fallback for headless CI / broken drivers.

## Hot-path rules

**Allowed on PTY → grid path:** VT parse, grid update, scrollback append, wake UI.

**Not allowed:** HTTP (Ollama), plugin WASM calls that block, file I/O, regex compilation, image decode.

Triggers: use **precompiled** regex only; if a rule is too slow, disable it after N timeouts.

## Measurement

- Optional `NEUTERM_PERF=1` logs frame times and PTY backlog.
- Prefer `tracing` spans over ad-hoc prints.
