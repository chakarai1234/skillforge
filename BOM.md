# Bill of Materials (BOM)
## Project: SkillForge CLI
**Version:** 1.0
**Date:** 2026-03-14
**Status:** Draft
**Versions fetched via:** Context7 MCP

---

## Overview

This document lists every technology, framework, library, and tool used in this project. Versions reflect the latest stable releases retrieved via Context7 MCP at the time of document generation. Pin all versions in your `Cargo.lock` before deploying to production.

---

## 1. Core Languages & Runtimes

| Package / Tool | Category | Latest Version | Docs |
|----------------|----------|----------------|------|
| Rust | Language | 1.86.0 (MSRV) | [Docs](https://doc.rust-lang.org/stable/) |
| Cargo | Build Tool & Package Manager | (ships with Rust) | [Docs](https://doc.rust-lang.org/cargo/) |

---

## 2. TUI & Terminal Dependencies

| Package / Tool | Category | Latest Version | Docs |
|----------------|----------|----------------|------|
| ratatui | TUI Framework | 0.30.0 | [Docs](https://ratatui.rs) |
| crossterm | Terminal Backend | 0.28.0 | [Docs](https://docs.rs/crossterm) |

---

## 3. Async & Networking

| Package / Tool | Category | Latest Version | Docs |
|----------------|----------|----------------|------|
| tokio | Async Runtime | 1.49.0 | [Docs](https://tokio.rs) |
| tokio-stream | Async Stream Utilities | 0.1.x | [Docs](https://docs.rs/tokio-stream) |
| reqwest | HTTP Client (SSE streaming) | 0.12.26 | [Docs](https://docs.rs/reqwest) |

---

## 4. Serialization & Config

| Package / Tool | Category | Latest Version | Docs |
|----------------|----------|----------------|------|
| serde | Serialization Framework | 1.0 | [Docs](https://serde.rs) |
| serde_json | JSON Serialization | 1.0 | [Docs](https://docs.rs/serde_json) |
| toml | TOML Config Parsing | 0.8.x | [Docs](https://docs.rs/toml) |

---

## 5. CLI & System Utilities

| Package / Tool | Category | Latest Version | Docs |
|----------------|----------|----------------|------|
| clap | CLI Argument Parsing | 4.5.x | [Docs](https://docs.rs/clap) |
| which | Executable PATH Discovery | 6.0.x | [Docs](https://docs.rs/which) |
| directories | Platform Config Directories | 6.0 | [Docs](https://docs.rs/directories) |
| arboard | Clipboard Read/Write | 3.x | [Docs](https://docs.rs/arboard) |

---

## 6. Content Processing

| Package / Tool | Category | Latest Version | Docs |
|----------------|----------|----------------|------|
| pulldown-cmark | Markdown Parser (CommonMark) | 0.12.x | [Docs](https://docs.rs/pulldown-cmark) |
| unicode-width | Unicode Character Width | 0.2.x | [Docs](https://docs.rs/unicode-width) |

---

## 7. Error Handling & Logging

| Package / Tool | Category | Latest Version | Docs |
|----------------|----------|----------------|------|
| anyhow | Application Error Handling | 1.0 | [Docs](https://docs.rs/anyhow) |
| tracing | Structured Logging Framework | 0.1.x | [Docs](https://docs.rs/tracing) |
| tracing-subscriber | Log Formatting & Filtering | 0.3.x | [Docs](https://docs.rs/tracing-subscriber) |

---

## 8. CI/CD & DevOps Tools

| Package / Tool | Category | Latest Version | Docs |
|----------------|----------|----------------|------|
| GitHub Actions | CI/CD Platform | — | [Docs](https://docs.github.com/en/actions) |
| cargo-dist | Cross-compilation & Release Automation | 0.28.x | [Docs](https://opensource.axo.dev/cargo-dist/) |
| cargo-nextest | Faster Test Runner | 0.9.x | [Docs](https://nexte.st) |
| cargo-audit | Dependency CVE Scanner | 0.21.x | [Docs](https://docs.rs/cargo-audit) |
| cross | Cross-compilation Toolchain | 0.2.x | [Docs](https://github.com/cross-rs/cross) |

---

## 9. Testing & Quality Tools

| Package / Tool | Category | Latest Version | Docs |
|----------------|----------|----------------|------|
| cargo test | Built-in Unit & Integration Testing | (ships with Rust) | [Docs](https://doc.rust-lang.org/cargo/commands/cargo-test.html) |
| clippy | Lint Tool | (ships with Rust) | [Docs](https://doc.rust-lang.org/clippy/) |
| rustfmt | Code Formatter | (ships with Rust) | [Docs](https://rust-lang.github.io/rustfmt/) |
| mockall | Mock Generation for Tests | 0.13.x | [Docs](https://docs.rs/mockall) |

---

## Notes

- All Rust crate versions fetched via Context7 MCP at document generation time (2026-03-14).
- Pin all versions in `Cargo.lock` — commit the lock file to source control for binaries.
- `crossterm` is re-exported by `ratatui` v0.30 — use `ratatui::crossterm` to avoid version conflicts.
- Use `reqwest` with the `rustls-tls` feature flag (not `native-tls`) for clean musl static linking.
- Run `cargo audit` in CI to detect CVEs in transitive dependencies.
- Re-generate BOM when upgrading major ratatui versions (breaking changes are common).
