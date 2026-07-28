# Plugins & Triggers

## Triggers (v1 priority)

Triggers transform or style terminal **output lines** when a string or regex matches — inspired by iTerm2 triggers.

### Rule schema

```yaml
triggers:
  enabled: true
  rules:
    - name: production-highlight
      match: "(?i)\\b(production|prod|prd)\\b"
      match_type: regex    # regex | string
      scope: line          # line = style whole line; match = style only the match
      style:
        foreground: "#ffffff"
        background: "#c0392b"
        # optional: bold, underline
      # future: action: highlight | annotate | notify | hyperlink
```

### Behavior

1. Rules compile at config load; invalid regex → skip rule + error log.
2. Applied on the **render path** as a styling overlay (or at line commit into the grid). Prefer decorating cells without rewriting PTY bytes so copy/paste stays clean unless an action explicitly mutates.
3. Multiple rules may apply; later rules can override style fields for overlapping regions (document order).
4. Triggers must be cheap: compiled `regex` crate, per-line evaluation, short-circuit when disabled.

### Example (user need)

If `production` appears, highlight the **entire line** red background / white foreground.

## Plugins (v1 stub → expand)

Plugins are code that extends NeuTerm beyond declarative triggers.

### Target model

- **Manifest + code file** discovered from `plugins.dirs` and optional built-in path.
- **Injection model (goal):** plugin code runs in a **sandbox** (WASM preferred) and can subscribe to events:
  - `on_line` / `on_output`
  - `on_key`
  - `on_tab_event`
  - `render_decorate` (return style spans — similar to triggers)
- Plugins must **not** block the PTY reader. Events are queued; slow plugins are dropped/throttled with a warning.

### v1 deliverable

- Config keys and directory layout documented
- Host stub that loads manifests and logs them
- Stable event trait so WASM host can land without breaking config

### Layout

```
~/.config/neuterm/plugins/
  my_plugin/
    plugin.yaml
    plugin.wasm   # or .rs source compiled by user — WASM is the runtime artifact
```

`plugin.yaml` example:

```yaml
name: my_plugin
version: 0.1.0
entry: plugin.wasm
permissions:
  - read_output
  - decorate
```

Native dynamic libraries (`.so`/`.dylib`/`.dll`) are **out of scope for v1** due to stability/safety; may be considered later for power users.

## Separation

| Feature | Declarative | Code | Hot path |
|---------|-------------|------|----------|
| Triggers | Yes (YAML) | No | Yes (must stay fast) |
| Plugins | Manifest | Yes (WASM) | No (async/event) |

If a trigger needs logic beyond style transforms, it should become a plugin.
