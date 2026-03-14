# Technical Design Document (TDD)
## Project: SkillForge CLI
**Version:** 1.0
**Date:** 2026-03-14
**Status:** Draft

---

## 1. System Overview

SkillForge CLI is a single Rust binary that combines terminal UI rendering, system PATH introspection, and AI API communication into one cohesive tool. The application uses an event-driven architecture with `tokio` for async I/O — keyboard events from `crossterm` are processed by the main event loop, while AI generation requests are dispatched as background async tasks that stream tokens back through an `mpsc` channel into the TUI render loop. The `ratatui` library handles all TUI widget rendering using an immediate-mode rendering model.

## 2. Architecture Diagram (Text / ASCII)

```
+------------------------------------------------------------+
|                    SkillForge CLI Binary                   |
|                                                            |
|  +--------------+   +----------------------------------+   |
|  |  CLI Args    |   |        TUI Event Loop            |   |
|  |  (clap)      |-->|  (tokio::select! + crossterm)    |   |
|  +--------------+   +-------------+--------------------+   |
|                                   |                         |
|             +---------------------v------------------+      |
|             |          App State Machine             |      |
|             |  +--------------+  +--------------+   |      |
|             |  | ToolList     |  | SkillPanel   |   |      |
|             |  | Panel State  |  | Panel State  |   |      |
|             |  +--------------+  +--------------+   |      |
|             +------------------+-----------------+         |
|                                |                            |
|  +-----------------------------v-----------------------+    |
|  |                  Service Layer                      |    |
|  |  +------------+  +------------+  +--------------+  |    |
|  |  | PathScanner|  | AIProvider |  | SkillStore   |  |    |
|  |  |  (which)   |  |  Trait     |  | (~/.config/  |  |    |
|  |  +------------+  +-----+------+  | skillforge/) |  |    |
|  +--------------------------------+--+--------------+  |    |
|                                   |                         |
|  +--------------------------------v---------------------+   |
|  |            AI Provider Implementations               |   |
|  |  +----------------------+  +----------------------+  |   |
|  |  | ClaudeProvider       |  | OpenAIProvider       |  |   |
|  |  | (Anthropic API)      |  | (OpenAI Chat API)    |  |   |
|  |  | reqwest + SSE        |  | reqwest + SSE        |  |   |
|  |  +----------------------+  +----------------------+  |   |
|  +------------------------------------------------------+   |
|                                                            |
|  +------------------------------------------------------+   |
|  |           Config Layer                               |   |
|  |  ~/.config/skillforge/config.toml                    |   |
|  |  ~/.config/skillforge/skills/<toolname>.md           |   |
|  +------------------------------------------------------+   |
+------------------------------------------------------------+
                            |
        External AI APIs    |
   +------------------------v------------------------------+
   |  https://api.anthropic.com/v1/messages               |
   |  https://api.openai.com/v1/chat/completions          |
   +-------------------------------------------------------+
```

## 3. Tech Stack

| Layer | Technology | Justification |
|-------|-----------|---------------|
| Language | Rust (stable 1.86.0+) | Memory safety, performance, single-binary distribution |
| TUI Framework | ratatui v0.30 | De-facto standard Rust TUI library; rich widgets, active maintenance |
| Terminal Backend | crossterm v0.28 | Cross-platform; re-exported by ratatui v0.30 |
| Async Runtime | tokio v1.49 (full features) | Required for concurrent AI streaming + event loop |
| HTTP Client | reqwest v0.12 (stream feature) | Ergonomic async HTTP with SSE/streaming support |
| Serialization | serde v1 + serde_json v1 | Ubiquitous in Rust; required for AI API request/response |
| CLI Argument Parsing | clap v4 (derive feature) | Standard CLI arg parsing; auto-generates help text |
| PATH Discovery | which v6 | Cross-platform binary resolution in PATH |
| Markdown Parsing | pulldown-cmark v0.12 | Lightweight CommonMark parser for skill validation |
| Config / Settings | toml v0.8 | Human-readable config; matches Cargo.toml familiarity |
| Platform Directories | directories v6 | Cross-platform config/data dir resolution |
| Error Handling | anyhow v1 | Idiomatic app-level error propagation |
| Clipboard | arboard v3 | Cross-platform clipboard write support |
| Async Streams | tokio-stream v0.1 | StreamExt utilities for SSE token streaming |
| Testing | cargo test + nextest | Built-in unit tests; nextest for faster CI runs |
| Linting/Formatting | clippy + rustfmt | Enforced in CI |
| Release Management | cargo-dist | Cross-compiles and publishes GitHub Release artifacts |

## 4. Component Design

### 4.1 App (Root State)
- Owns all panel states and the active focus
- Drives the main render loop via `ratatui::Terminal::draw()`
- Listens to a `tokio::sync::mpsc::Receiver<AppEvent>` for both keyboard events and AI stream tokens
- State machine transitions: `Idle -> Generating -> Ready -> Installing`

### 4.2 ToolListPanel (Left Column)
- **Responsibilities:** Render the scrollable list of CLI tools; handle selection toggling; manage search filter state
- **Key fields:** `tools: Vec<ToolEntry>`, `selected: HashSet<String>`, `filter: String`, `scroll_offset: usize`
- **ToolEntry:** `name: String`, `path: PathBuf`, `has_skill: bool`
- Populated at startup by `PathScanner::scan()`; re-scanned on `r` key

### 4.3 SkillPanel (Right Column)
- **Top row — RequirementInput:** Single-line text input widget; captures keystrokes when focused; sends `GenerateRequest` on `Enter`
- **Bottom row — SkillOutput:** Scrollable text area rendering streamed markdown; wraps lines at panel width
- **Action bar:** Renders `[i] Install [c] Copy [r] Regen` shortcuts; active only when output is non-empty

### 4.4 PathScanner (Service)
- Uses the `which` crate iteratively across `$PATH` entries
- Deduplicates by binary name; sorts alphabetically
- Cross-references `~/.config/skillforge/skills/` to populate `has_skill` flag
- Runs in a `tokio::task::spawn_blocking` call at startup

### 4.5 AIProvider Trait

```rust
#[async_trait]
pub trait AIProvider: Send + Sync {
    async fn generate_skill(
        &self,
        tool_name: &str,
        requirement: &str,
        tx: mpsc::Sender<StreamToken>,
    ) -> Result<()>;
}

pub enum StreamToken {
    Token(String),
    Done,
    Error(String),
}
```

- `ClaudeProvider`: POST to `https://api.anthropic.com/v1/messages` with `stream: true`; parses SSE `data:` lines
- `OpenAIProvider`: POST to `https://api.openai.com/v1/chat/completions` with `stream: true`; parses SSE `data:` lines

### 4.6 SkillStore (Service)
- Abstracts read/write of skill files on disk
- `install(tool: &str, content: &str) -> Result<PathBuf>`
- `exists(tool: &str) -> bool`
- `load(tool: &str) -> Result<String>`
- Default path: `~/.config/skillforge/skills/<tool>.md`; overridable in config

### 4.7 Config

```toml
# ~/.config/skillforge/config.toml

[provider]
name = "claude"           # "claude" | "openai" | "custom"
model = "claude-sonnet-4-6"
api_key_env = "ANTHROPIC_API_KEY"
base_url = ""             # optional override for custom endpoints

[skills]
output_dir = ""           # defaults to ~/.config/skillforge/skills/

[ui]
color_theme = "yellow"    # reserved for future themes
```

## 5. Database Design

Not applicable — SkillForge uses the filesystem for persistence. Installed skill files are stored as individual markdown files in `~/.config/skillforge/skills/<toolname>.md`. An in-memory index is built at startup by scanning this directory.

## 6. API Design (AI Provider Endpoints)

### Claude (Anthropic) — Messages API

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `https://api.anthropic.com/v1/messages` | Generate a skill using Claude with streaming SSE |

**Request body:**
```json
{
  "model": "claude-sonnet-4-6",
  "max_tokens": 2048,
  "stream": true,
  "system": "You are an expert at writing CLI skill definitions in markdown format...",
  "messages": [
    { "role": "user", "content": "Generate a skill for the tool '<tool>' that: <requirement>" }
  ]
}
```

**Auth:** `x-api-key: $ANTHROPIC_API_KEY`, `anthropic-version: 2023-06-01`

### OpenAI — Chat Completions API

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `https://api.openai.com/v1/chat/completions` | Generate a skill using GPT-4o with streaming SSE |

**Request body:**
```json
{
  "model": "gpt-4o",
  "stream": true,
  "messages": [
    { "role": "system", "content": "You are an expert at writing CLI skill definitions..." },
    { "role": "user", "content": "Generate a skill for '<tool>' that: <requirement>" }
  ]
}
```

**Auth:** `Authorization: Bearer $OPENAI_API_KEY`

**Error handling conventions:**
- `401` -> surface "Invalid API key — check your environment variable"
- `429` -> surface "Rate limited — please wait before retrying"
- `5xx` -> surface "Provider error — try again or switch providers"
- Network timeout -> surface "Connection timed out" with retry prompt

**Skill generation system prompt template:**
```
You are an expert at writing AI skill definitions for CLI tools.
A skill is a markdown document that teaches an AI assistant how to
help with a specific CLI tool. It should include:
1. A brief description of the tool
2. Common workflows and commands
3. Best practices and gotchas
4. Example prompts that work well with the tool

Generate a skill for: {tool_name}
User's specific requirement: {requirement}

Output ONLY the markdown content, no explanations.
```

## 7. Frontend Architecture (TUI)

### Framework
- **ratatui v0.30** with **crossterm v0.28** backend
- Immediate-mode rendering: `terminal.draw(|frame| app.render(frame))` called on every event

### Rendering Pipeline
```
tokio::select! {
    event = crossterm_event_stream.next() => handle_input(event),
    token = ai_rx.recv()                 => handle_ai_token(token),
}
-> app.update(event_or_token)
-> terminal.draw(|frame| render(frame, &app))
```

### Component Structure
```
App
+-- TitleBar         (top bar: app name, version, active provider)
+-- ToolListPanel    (left column, 30% width)
|   +-- SearchBar    (top of left column)
|   +-- ToolList     (scrollable, with checkboxes and checkmark indicators)
|   +-- ActionButton ("Generate Skills" at bottom of left column)
+-- SkillPanel       (right column, 70% width)
|   +-- RequirementInput  (top 25% of right column)
|   +-- SkillOutput       (bottom 75% of right column, scrollable)
|   +-- ActionBar         ([i]Install [c]Copy [r]Regen)
+-- HelpOverlay      (modal, toggled by ?)
```

### Focus Management
- `Tab` cycles focus: ToolList -> RequirementInput -> SkillOutput
- Active panel border rendered in bright yellow (`Color::LightYellow`)
- Inactive panel border rendered in dark yellow (`Color::Yellow` dim)

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Tab` | Cycle focus between panels |
| Up/Down or j/k | Navigate tool list |
| `Space` | Toggle tool selection checkbox |
| `Enter` (in input) | Trigger AI generation |
| `i` | Install currently displayed skill |
| `c` | Copy skill to clipboard |
| `r` | Regenerate skill with same requirement |
| `/` | Jump to search bar |
| `Esc` | Clear search / cancel generation |
| `?` | Toggle help overlay |
| `q` | Quit |

## 7a. Design System & Branding

- **UI inspiration:** Retro terminal aesthetic, developer-first density
- **UI style:** Dark terminal background, yellow monochrome accent scheme

**Color Palette (ratatui `Color` mappings):**

| Role | ratatui Color | Hex Equivalent |
|------|--------------|----------------|
| Primary border / title | `Color::Yellow` | `#FFD700` |
| Active / focused border | `Color::LightYellow` | `#FFFF00` |
| Button / CTA | `Color::Rgb(255,165,0)` (Amber) | `#FFA500` |
| Selected list item bg | `Color::Rgb(60,50,0)` | — |
| Installed indicator | `Color::LightGreen` | `#00FF7F` |
| Normal text | `Color::White` | `#E0E0E0` |
| Dimmed / secondary text | `Color::DarkGray` | — |
| Error text | `Color::LightRed` | — |

**Typography:** Monospace terminal font (user's terminal font). Bold used for titles and active selections; dim for secondary metadata.

**Spacing:** ratatui `Constraint::Percentage` splits — left panel 30%, right panel 70%; right panel split 25% requirement / 75% output.

**Component library:** ratatui built-in widgets (`List`, `Paragraph`, `Block`, `Borders`, `Gauge` for streaming progress).

## 8. Infrastructure Setup

**Distribution model:** Single statically linked binary distributed via:
1. **crates.io** — `cargo install skillforge`
2. **GitHub Releases** — Pre-compiled binaries for all targets (via `cargo-dist`)
3. **Homebrew tap** — `brew install skillforge` (phase 2)

**Build targets (cross-compiled in CI):**

| Target | Platform |
|--------|----------|
| `x86_64-unknown-linux-musl` | Linux x86_64 (static) |
| `aarch64-unknown-linux-musl` | Linux ARM64 (static) |
| `x86_64-apple-darwin` | macOS Intel |
| `aarch64-apple-darwin` | macOS Apple Silicon |
| `x86_64-pc-windows-msvc` | Windows x86_64 |

**Config & data directories (via `directories` crate):**

| Path | Purpose |
|------|---------|
| `~/.config/skillforge/config.toml` | User config |
| `~/.config/skillforge/skills/` | Installed skill files |

**Secrets management:** API keys read exclusively from environment variables at runtime. Never written to disk by the app.

## 9. CI/CD Pipeline (GitHub Actions)

**Branch strategy:** Trunk-based — PRs to `main`; feature branches short-lived.

**Pipeline stages:**

```yaml
# .github/workflows/ci.yml
on: [push, pull_request]
jobs:
  lint:    clippy --all-targets --all-features
  format:  rustfmt --check
  test:    cargo nextest run
  build:   cargo build --release (matrix: all 5 targets)

# .github/workflows/release.yml (triggered by git tag v*)
jobs:
  dist: cargo-dist build -> uploads binaries to GitHub Release
  publish: cargo publish -> pushes to crates.io
```

**Environment promotion:** Direct trunk-based release — tag `v*` triggers release pipeline.

**Rollback strategy:** Yanked crates.io versions + GitHub Release marked as pre-release until verified.

## 10. Security Considerations

- **API keys:** Read-only from env vars; never logged, printed, or persisted
- **Network:** All AI API calls over HTTPS; `reqwest` validates TLS certificates by default
- **Input validation:** Requirement input text sanitized before embedding in API request (no prompt injection via special characters)
- **Skill output:** Written only to user-controlled config directory; no shell execution of generated content
- **Dependencies:** `cargo audit` run in CI to detect known CVEs in the dependency tree
- **Binary integrity:** GitHub Release checksums (SHA256) published alongside binaries; cargo-dist generates them automatically

## 11. Scalability & Performance

- **PATH scanning:** Runs in `spawn_blocking` to avoid blocking the async event loop; completes in < 500ms for typical systems
- **Tool list rendering:** Virtual scrolling via ratatui `List` widget with scroll offset — only visible rows rendered
- **AI streaming:** SSE tokens written to an `mpsc::channel`; UI re-renders per token for immediacy; channel bounded to 256 to apply backpressure
- **Memory:** Each skill file is <= 8KB; storing 500 tool entries in memory is negligible (< 1MB)
- **Startup time:** No network calls at startup; PATH scan + config read complete in < 200ms on modern hardware

## 12. Observability & Monitoring

- **Logging:** `tracing` crate with `tracing-subscriber`; log level controlled by `RUST_LOG` env var; logs written to `~/.config/skillforge/skillforge.log` (not to stdout to avoid TUI corruption)
- **Errors:** `anyhow` error chains surfaced in TUI status bar; full chain in log file
- **No telemetry:** SkillForge collects zero usage data or telemetry

## 13. Open Questions & Risks

| Question / Risk | Mitigation |
|-----------------|-----------|
| Some PATH tools are system binaries (ls, cat) — generating skills for them is low value | Add a default denylist for common system utilities; allow user to override |
| AI providers change their streaming API formats | Abstract SSE parsing per-provider; version-pin API format in provider config |
| ratatui mouse support varies across terminal emulators | Keyboard-only as primary interaction model; mouse as optional enhancement |
| Cross-compilation of reqwest (TLS) on musl targets requires extra setup | Use `rustls` feature flag instead of system OpenSSL for static linking |
| Windows console codepage may not render Unicode box-drawing chars | Test on Windows Terminal; fall back to ASCII borders if VT mode unavailable |

## 14. Glossary

| Term | Definition |
|------|-----------|
| Skill | A structured markdown file that teaches an AI assistant how to work with a specific CLI tool |
| TUI | Terminal User Interface — a text-based graphical interface running in a terminal emulator |
| PATH | The system environment variable listing directories searched for executable programs |
| SSE | Server-Sent Events — an HTTP streaming technique used by AI APIs to stream tokens |
| MSRV | Minimum Supported Rust Version — the oldest Rust version that can compile the project |
| Provider | An AI service (Claude, OpenAI) that SkillForge connects to for skill generation |
| Immediate-mode rendering | A UI paradigm where the full frame is redrawn on every event rather than maintaining a stateful DOM |
