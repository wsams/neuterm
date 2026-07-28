# AI Agents (Ollama)

## Intent

NeuTerm should support AI agents natively, starting with **Ollama**. Keep it smart, simple, and non-intrusive.

v1 scope:

1. Configure an Ollama instance (host, port, model, timeouts).
2. **Chat pane** — simple conversational UI beside the terminal.
3. **Command help** — shortcut opens a small overlay: ask for a command, get a suggestion, accept to replace the current input line.

Out of scope for v1: multi-agent orchestration, tool-calling into the PTY without confirmation, embedding a full Open WebUI clone.

## Configuration

```yaml
ai:
  enabled: true
  ollama:
    host: "127.0.0.1"
    port: 11434
    base_url: null          # e.g. "http://192.168.1.10:11434" overrides host/port
    model: "llama3.2"
    timeout_ms: 60000
  command_help:
    shortcut: "default"
    system_prompt: null     # optional override
  chat:
    title: "AI Chat"
```

Resolved API base: `base_url` or `http://{host}:{port}`.

Standard Ollama endpoints used:

- `GET /api/tags` — connectivity / model list (optional UI)
- `POST /api/chat` — chat + command help (streaming preferred)

## Chat pane

- Action **Open AI Chat** splits (or focuses) a `PaneKind::AiChat` leaf.
- UI: message list + input box; streams tokens as they arrive.
- Does not steal keystrokes from the terminal unless focused.
- Errors (connection refused, missing model) show inline, not as modal spam.

### Embedded chat harness research note

Projects like [Open WebUI](https://github.com/open-webui/open-webui) and similar “chat harness” UIs are typically full web apps. Embedding options:

| Approach | Pros | Cons |
|----------|------|------|
| Native UI (egui/iced/custom) | Fast, consistent, offline | Rebuild chat UX |
| Embedded webview → Open WebUI | Rich features | Heavy, packaging pain, less “simple & fast” |
| Lightweight HTTP client + native widgets | Matches NeuTerm goals | Fewer features |

**Decision:** v1 uses a **native lightweight chat pane** talking to Ollama’s HTTP API. A future optional webview panel may embed Open WebUI / similar if users want the full harness — not the default.

## Command help (default AI UX)

Non-intrusive flow:

1. User presses shortcut (default **Super+;** or **Super+/** — see keybindings).
2. Small overlay appears near the cursor / bottom of the active pane.
3. User types a natural-language request (“tar.gz extract preserving perms”).
4. NeuTerm sends a constrained prompt to Ollama asking for a **single shell command** (+ brief explanation).
5. UI shows suggestion; keys:
   - **Enter** — replace the current prompt line / paste into terminal input
   - **Esc** — dismiss, no changes
   - **Tab** — insert without submitting to shell (if distinguishable)

Never auto-execute suggested commands in v1.

### Prompt sketch

```
You are a command-line assistant. Reply with:
1) a single shell command for the user's OS
2) one short sentence of explanation
Do not wrap the command in markdown fences if avoidable.
```

Include OS hint (`linux`/`macos`/`windows`) and optional shell name from config.

## Safety

- AI features off unless `ai.enabled: true`.
- No secrets from the terminal buffer are sent unless the user explicitly includes selection / “explain this” (future).
- Command help sends only the user's question (+ OS/shell metadata), not full scrollback, by default.

## Future

- “Explain selection”
- Agent profiles / multiple models
- Optional Open WebUI embed
- Tool use with explicit confirm
