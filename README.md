<div align="center">

# SkillForge CLI

**AI-powered skill generator for CLI tools — keyboard-driven terminal UI**

[![CI](https://img.shields.io/github/actions/workflow/status/chakarai1234/skillforge/ci.yml?branch=main&label=CI&logo=githubactions&logoColor=white&color=brightgreen&style=flat-square)](https://github.com/chakarai1234/skillforge/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/actions/workflow/status/chakarai1234/skillforge/release.yml?label=Release&logo=githubactions&logoColor=white&color=brightgreen&style=flat-square)](https://github.com/chakarai1234/skillforge/actions/workflows/release.yml)
[![Latest Release](https://img.shields.io/github/v/release/chakarai1234/skillforge?color=orange&label=Latest&logo=github&style=flat-square)](https://github.com/chakarai1234/skillforge/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=flat-square)](LICENSE)
[![Rust MSRV](https://img.shields.io/badge/rust-1.86.0%2B-orange?logo=rust&style=flat-square)](https://www.rust-lang.org)

<br/>

![SkillForge Demo](https://via.placeholder.com/900x500/1a1a1a/FFA500?text=SkillForge+TUI+Demo)

</div>

---

## What is SkillForge?

SkillForge is a keyboard-driven terminal UI that generates **AI skill markdown files** for popular CLI tools. Skill files teach an AI assistant how to help with a specific tool — describing workflows, best practices, and example prompts — then installs them to `~/.skillforge/skills/` where your AI coding assistant can pick them up.

It supports **four AI providers** (Claude, OpenAI, Gemini, OpenRouter) with streaming output, live model listing from each provider's API, and automatic config persistence.

---

## CI / CD Status

| Workflow | Status | Trigger |
|----------|--------|---------|
| **Lint** — `rustfmt` + `clippy -D warnings` | [![Lint](https://img.shields.io/github/actions/workflow/status/chakarai1234/skillforge/ci.yml?branch=main&label=lint&style=flat-square&color=brightgreen)](https://github.com/chakarai1234/skillforge/actions/workflows/ci.yml) | Push / PR to `main` |
| **Test** — `cargo test --all` | [![Test](https://img.shields.io/github/actions/workflow/status/chakarai1234/skillforge/ci.yml?branch=main&label=test&style=flat-square&color=brightgreen)](https://github.com/chakarai1234/skillforge/actions/workflows/ci.yml) | Push / PR to `main` |
| **Build Check** — 4 targets | [![Build](https://img.shields.io/github/actions/workflow/status/chakarai1234/skillforge/ci.yml?branch=main&label=build-check&style=flat-square&color=brightgreen)](https://github.com/chakarai1234/skillforge/actions/workflows/ci.yml) | After lint + test pass |
| **Release** — 5 targets cross-compiled | [![Release](https://img.shields.io/github/actions/workflow/status/chakarai1234/skillforge/release.yml?label=release&style=flat-square&color=brightgreen)](https://github.com/chakarai1234/skillforge/actions/workflows/release.yml) | On `v*` tag push |

> **Badge colours:**
> - ![green](https://img.shields.io/badge/-passing-brightgreen?style=flat-square) **Green** — all checks passed
> - ![amber](https://img.shields.io/badge/-warning-orange?style=flat-square) **Amber** — skipped steps or non-blocking notices
> - ![red](https://img.shields.io/badge/-failing-red?style=flat-square) **Red** — build, lint, or test failure — do not ship

---

## Release Targets

Every tagged release (`v*.*.*`) automatically cross-compiles five binaries and publishes them as GitHub Release assets.

| Platform | Architecture | Binary | Tool |
|----------|-------------|--------|------|
| macOS | Apple Silicon (M1/M2/M3) | `skillforge-macos-arm64` | Native `cargo` on `macos-14` |
| macOS | Intel x86_64 | `skillforge-macos-x86_64` | Native `cargo` on `macos-13` |
| Linux | x86_64 (static musl) | `skillforge-linux-x86_64` | `cargo` + `musl-tools` |
| Linux | ARM64 (static musl) | `skillforge-linux-arm64` | `cross` via Docker |
| Windows | x86_64 MSVC | `skillforge-windows-x86_64.exe` | Native `cargo` on `windows-latest` |

Each binary ships with a `.sha256` checksum file. Verify before installing:

```bash
sha256sum -c skillforge-linux-x86_64.sha256
```

---

## Installation

### macOS — Apple Silicon (M1/M2/M3)

```bash
curl -Lo skillforge \
  https://github.com/chakarai1234/skillforge/releases/latest/download/skillforge-macos-arm64
chmod +x skillforge
sudo mv skillforge /usr/local/bin/
```

### macOS — Intel

```bash
curl -Lo skillforge \
  https://github.com/chakarai1234/skillforge/releases/latest/download/skillforge-macos-x86_64
chmod +x skillforge
sudo mv skillforge /usr/local/bin/
```

### Linux — x86_64

```bash
curl -Lo skillforge \
  https://github.com/chakarai1234/skillforge/releases/latest/download/skillforge-linux-x86_64
chmod +x skillforge
sudo mv skillforge /usr/local/bin/
```

### Linux — ARM64

```bash
curl -Lo skillforge \
  https://github.com/chakarai1234/skillforge/releases/latest/download/skillforge-linux-arm64
chmod +x skillforge
sudo mv skillforge /usr/local/bin/
```

### Windows — x86_64

Download [`skillforge-windows-x86_64.exe`](https://github.com/chakarai1234/skillforge/releases/latest/download/skillforge-windows-x86_64.exe) from the Releases page and add the folder to your `PATH`.

### Build from Source

Requires Rust 1.86.0+.

```bash
git clone https://github.com/chakarai1234/skillforge.git
cd skillforge
cargo build --release
sudo cp target/release/skillforge /usr/local/bin/
```

---

## Quick Start

### 1 — Set your API key

SkillForge reads API keys from environment variables only. Keys are **never** written to disk.

```bash
# Pick one (or more — you can switch providers in the TUI)
export ANTHROPIC_API_KEY="sk-ant-..."
export OPENAI_API_KEY="sk-..."
export GEMINI_API_KEY="AIza..."
export OPENROUTER_API_KEY="sk-or-..."
```

### 2 — Launch

```bash
skillforge
# with a custom config path
skillforge --config ~/dotfiles/skillforge.toml
```

### 3 — Generate a skill

1. Press `1` to open the **Skills** tab (default).
2. Navigate with `↑`/`↓` or `j`/`k` to highlight a tool.
3. Press `Tab` to jump to the **Skill Name** field (optional — defaults to the tool name).
4. Press `Tab` again to reach the **Requirement** field. Describe what the skill should cover.
5. Press `Enter` to start streaming generation.
6. Press `i` to install the file to `~/.skillforge/skills/`.

---

## Features

### Skills Tab

| Feature | Detail |
|---------|--------|
| Curated tool list | `codex`, `claude-code`, `gemini-cli`, `opencode`, `copilot-cli` |
| Multi-select | `Space` toggles; generate skills for multiple tools at once |
| Filter bar | `/` to fuzzy-filter the list in real time |
| Skill Name field | Optional custom filename at the top of the right panel |
| Skills folder shown | `~/.skillforge/skills/` displayed in the panel title |
| Streaming output | Tokens stream live as the AI generates |
| Scrollable output | `↑`/`↓` scrolls the generated markdown with a scrollbar |
| Install | `i` writes the skill file to `~/.skillforge/skills/<name>.md` |
| Copy | `c` copies the output to the system clipboard |
| Regenerate | `r` re-runs generation with the same tool + requirement |
| Focused borders | Yellow border = active pane · DarkGray = inactive |

### Providers Tab

| Feature | Detail |
|---------|--------|
| 4 providers | Claude, OpenAI, Gemini, OpenRouter |
| API key entry | Type directly in the TUI — masked by default, `Ctrl+H` to reveal |
| Live model listing | Fetches available models from each provider's REST API |
| Model navigator | `◀` / `▶` to cycle through fetched models |
| Config persistence | Active provider and model saved to `~/.skillforge/config.toml` |
| Key security | API keys are **never** written to the config file |

---

## AI Providers

| Provider | API Key Env Var | Model Navigator | Streaming |
|----------|----------------|-----------------|-----------|
| **Anthropic Claude** | `ANTHROPIC_API_KEY` | `GET /v1/models` | Messages SSE |
| **OpenAI** | `OPENAI_API_KEY` | `GET /v1/models` (GPT/o-series only) | Chat Completions SSE |
| **Google Gemini** | `GEMINI_API_KEY` | `GET /v1beta/models` | `streamGenerateContent` SSE |
| **OpenRouter** | `OPENROUTER_API_KEY` | `GET /api/v1/models` (top providers, max 60) | OpenAI-compatible SSE |

---

## Keyboard Reference

### Global

| Key | Action |
|-----|--------|
| `1` | Switch to Skills tab |
| `2` | Switch to Providers tab |
| `Tab` / `Shift+Tab` | Cycle panel focus |
| `q` | Quit |
| `?` | Toggle help overlay |

### Skills Tab

| Key | Focus | Action |
|-----|-------|--------|
| `↑` `↓` / `j` `k` | Tool list | Navigate |
| `Space` | Tool list | Toggle selection |
| `/` | Anywhere | Jump to filter bar |
| `Tab` | Tool list | Focus → Skill Name |
| `Tab` | Skill Name | Focus → Requirement |
| `Tab` | Requirement | Focus → Output |
| `Tab` | Output | Focus → Tool list |
| `Enter` | Tool list / Requirement | Generate skill |
| `i` | Output | Install skill to disk |
| `c` | Output | Copy to clipboard |
| `r` | Output | Regenerate |
| `↑` `↓` | Output | Scroll generated markdown |
| `Esc` | Search / Requirement | Clear field |

### Providers Tab

| Key | Focus | Action |
|-----|-------|--------|
| `↑` `↓` | Provider list | Navigate |
| `Enter` | Provider list | Activate + open config |
| `Tab` | Provider list | Jump to API Key field |
| `Ctrl+H` | API Key field | Toggle key visibility |
| `Enter` | API Key / Model field | Save & activate |
| `Tab` | API Key field | Jump to Model field |
| `◀` `▶` | Model field | Cycle fetched models |
| `Tab` | Model field | Back to provider list |
| `Esc` | Any field | Back to provider list |

---

## Configuration

SkillForge stores its config at `~/.skillforge/config.toml`. Only the active **provider name** and **model** are persisted — API keys are always sourced from environment variables.

```toml
[provider]
name  = "claude"
model = "claude-sonnet-4-20250514"
```

### Skills directory

Generated skill files are installed to:

```
~/.skillforge/skills/
├── codex.md
├── claude-code.md
└── gemini-cli.md
```

Use `--config` to override the config file location:

```bash
skillforge --config /etc/skillforge/config.toml
```

---

## Project Structure

```
skillforge/
├── src/
│   ├── main.rs              # Entry point, event loop, tokio::select!
│   ├── app.rs               # App state, keyboard handling, skill install
│   ├── config.rs            # ~/.skillforge/config.toml load/save
│   ├── types.rs             # Shared enums (Focus, AppTab, StreamToken, …)
│   ├── ui/
│   │   └── mod.rs           # ratatui rendering — all panels & overlays
│   ├── providers/
│   │   ├── mod.rs           # AIProvider trait, build_provider, fetch_provider_models
│   │   ├── claude.rs        # Anthropic Messages API + model listing
│   │   ├── openai.rs        # OpenAI Chat Completions API + model listing
│   │   ├── gemini.rs        # Gemini streamGenerateContent + model listing
│   │   └── openrouter.rs    # OpenRouter (OpenAI-compatible) + model listing
│   └── services/
│       └── path_scanner.rs  # Curated tool list
├── .github/
│   └── workflows/
│       ├── ci.yml           # Lint → Test → Build-check (4 targets)
│       └── release.yml      # Cross-compile 5 targets → GitHub Release
└── Cargo.toml
```

---

## CI Pipeline Detail

```
Push / PR to main
       │
       ├── lint ──────────── cargo fmt --check
       │                     cargo clippy -D warnings
       │
       ├── test ──────────── cargo test --all
       │
       └── build-check ───── (needs: lint + test)
                ├── x86_64-unknown-linux-musl   (ubuntu-latest)
                ├── x86_64-apple-darwin         (macos-13)
                ├── aarch64-apple-darwin        (macos-14)
                └── x86_64-pc-windows-msvc      (windows-latest)
```

```
Push tag  v*.*.*
       │
       └── build ─────────── parallel matrix
                ├── skillforge-linux-x86_64      musl static
                ├── skillforge-linux-arm64        musl static (cross)
                ├── skillforge-macos-x86_64       native macos-13
                ├── skillforge-macos-arm64         native macos-14
                └── skillforge-windows-x86_64.exe native windows
                        │
                        └── release ─── download artifacts
                                        create GitHub Release
                                        attach binaries + .sha256 files
                                        auto-generate release notes
```

### Status indicators

| Colour | Meaning |
|--------|---------|
| ![green](https://img.shields.io/badge/-green-brightgreen?style=flat-square) | Job passed — safe to merge / ship |
| ![amber](https://img.shields.io/badge/-amber-orange?style=flat-square) | Warning or skipped steps — review before shipping |
| ![red](https://img.shields.io/badge/-red-red?style=flat-square) | Job failed — **do not merge / release** |

---

## Releases

All releases are published on the [GitHub Releases](https://github.com/chakarai1234/skillforge/releases) page.

Each release includes:

- **5 pre-built binaries** (see table above)
- **SHA256 checksum** file for every binary
- **Auto-generated release notes** from commit history (GitHub)
- **One-line install commands** for every platform in the release body

### Creating a release

```bash
git tag v1.2.0
git push origin v1.2.0
```

The Release workflow triggers automatically. It:

1. Compiles all 5 targets in parallel (Linux ARM64 via `cross` + Docker).
2. Packages each binary and generates a `.sha256`.
3. Uploads artifacts to GitHub.
4. Creates a draft-free GitHub Release with install instructions embedded.

---

## Tech Stack

| Crate | Version | Purpose |
|-------|---------|---------|
| `ratatui` | 0.29 | Terminal UI rendering |
| `crossterm` | 0.28 | Cross-platform terminal control + event stream |
| `tokio` | 1.x | Async runtime, channels, task spawning |
| `reqwest` | 0.12 | HTTP client with SSE streaming (`rustls-tls`) |
| `serde` / `serde_json` | 1.x | JSON serialisation for API bodies |
| `toml` | 0.8 | Config file serialisation |
| `clap` | 4.x | CLI argument parsing |
| `arboard` | 3.x | Cross-platform clipboard |
| `async-trait` | 0.1 | Async methods on `AIProvider` trait |
| `futures-util` | 0.3 | `StreamExt` for SSE byte stream iteration |
| `anyhow` | 1.x | Error handling |
| `tracing` | 0.1 | Structured logging to `~/.skillforge/skillforge.log` |

---

## License

MIT — see [LICENSE](LICENSE).

---

<div align="center">

Built with Rust · Runs entirely in your terminal · No telemetry · No data leaves your machine (except API calls you initiate)

</div>
