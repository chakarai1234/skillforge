# Product Requirements Document (PRD)
## Project: SkillForge CLI
**Version:** 1.0
**Date:** 2026-03-14
**Status:** Draft

---

## 1. Executive Summary

SkillForge CLI is a Rust-based terminal user interface (TUI) application that helps developers discover CLI tools installed on their system and generate AI-powered skill files for each of them on demand. Users describe what they want a skill to accomplish, and SkillForge connects to a configurable AI provider (Claude, OpenAI, or any compatible API) to generate a structured markdown skill definition. The result is a fast, keyboard-driven terminal workflow for building, previewing, and installing AI skills without leaving the terminal.

## 2. Problem Statement

AI-assisted developer tools such as Claude Code rely on "skills" — structured markdown prompt templates that define how the AI should behave when working with a given CLI tool. Creating these skill files manually is time-consuming, inconsistent, and requires expertise in both the target tool and prompt engineering. Developers who rely on many CLI tools have no unified, efficient way to generate, preview, and install skills for all of them at once. The gap between the number of tools a developer uses and the number of skills they have configured represents significant unrealized value from their AI assistant.

## 3. Goals & Objectives

**Primary goal:** Provide a unified TUI interface to discover installed CLI tools, generate AI skills for them, and install the results to the local AI configuration directory.

**Secondary goals:**
- Support configurable AI providers (Claude, OpenAI, and custom endpoints via config)
- Allow users to preview generated skill markdown before committing to install
- Stream AI responses in real-time for immediate feedback
- Persist skill configurations in a cross-platform standard directory

**Non-goals (v1):**
- This app will NOT manage or rotate AI API credentials
- This app will NOT sync skills to remote servers or skill marketplaces
- This app will NOT provide a built-in skill editor beyond the requirement input field
- This app will NOT integrate directly with IDE plugins or language servers

## 4. Target Users / Personas

### Persona 1: The Power Developer
A senior software engineer with 20+ CLI tools installed daily (git, docker, kubectl, terraform, gh, cargo, npm, etc.). Wants to rapidly generate skills for all tools in their stack without writing prompt templates by hand.

### Persona 2: The AI Tooling Enthusiast
An early adopter who uses Claude Code or similar AI CLI assistants daily. Wants to extend their AI assistant's knowledge with custom, high-quality skills for every tool in their workflow. Follows new AI tooling closely and installs new skills frequently.

### Persona 3: The Platform / DevOps Engineer
Manages many infrastructure tools (helm, aws, ansible, pulumi, terraform). Needs skills that encode organization-specific workflows. Values speed, repeatability, and a terminal-native experience.

## 5. User Stories

1. As a developer, I want to see a list of all CLI tools detected on my system PATH, so that I know which ones I can generate skills for.
2. As a developer, I want to check/select CLI tools from the list, so that I can queue multiple skills for generation.
3. As a developer, I want to type a natural-language requirement describing what the skill should do, so that the AI can generate a targeted, relevant skill.
4. As a developer, I want to see the generated skill markdown rendered live in the TUI, so that I can review it before installing.
5. As a developer, I want to press a single key to install a generated skill, so that the markdown file is saved immediately to my local AI config directory.
6. As a developer, I want to configure which AI provider and model to use via a config file or environment variable, so that I can use my preferred AI backend.
7. As a developer, I want to see which tools already have an installed skill (with a visual indicator), so that I can focus on tools that don't yet have one.
8. As a developer, I want to regenerate a skill with a new requirement, so that I can iterate on the skill definition without restarting the app.
9. As a developer, I want to navigate the entire TUI with the keyboard, so that I do not need a mouse.
10. As a developer, I want to filter the CLI tools list by typing a search term, so that I can quickly find a specific tool among hundreds.
11. As a developer, I want to copy the generated skill markdown to the clipboard, so that I can paste it into another tool or file.
12. As a developer, I want the AI response to stream token by token into the output panel, so that I get immediate visual feedback without waiting for the full generation.

## 6. Functional Requirements

### Must Have
| ID | Requirement | Priority |
|----|-------------|----------|
| F1 | Discover and list all executable binaries found in the system PATH | Must Have |
| F2 | Split-panel TUI layout: left column (tool list), right column (requirement input + markdown output) | Must Have |
| F3 | Multi-select tool list with checkbox-style selection | Must Have |
| F4 | Requirement text input field in the right panel top row | Must Have |
| F5 | AI skill generation via configurable HTTP API (Claude / OpenAI) | Must Have |
| F6 | Streaming markdown output rendered in the right panel bottom row | Must Have |
| F7 | Install skill: save generated markdown to `~/.config/skillforge/skills/<toolname>.md` | Must Have |
| F8 | Yellow color theme throughout the TUI | Must Have |
| F9 | Support Anthropic Claude and OpenAI API providers | Must Have |
| F10 | Read API keys from environment variables (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`) | Must Have |

### Should Have
| ID | Requirement | Priority |
|----|-------------|----------|
| F11 | Real-time streaming token rendering in the output panel | Should Have |
| F12 | Search / filter bar above the CLI tools list | Should Have |
| F13 | Keyboard shortcut help overlay (press `?`) | Should Have |
| F14 | Installed skill status indicator (checkmark) next to tool names | Should Have |
| F15 | Config file (`~/.config/skillforge/config.toml`) for provider, model, output directory | Should Have |

### Could Have
| ID | Requirement | Priority |
|----|-------------|----------|
| F16 | Copy generated skill to clipboard | Could Have |
| F17 | Skill diff view when regenerating an existing skill | Could Have |
| F18 | Configurable skill output path (default: `~/.claude/skills/` for Claude Code compatibility) | Could Have |

### Won't Have (v1)
- Remote skill sync or community sharing
- Built-in markdown editor
- Plugin architecture

## 7. Non-Functional Requirements

- **Performance:** App must start and render the initial screen within 200ms. PATH scanning must complete within 500ms for systems with up to 500 tools.
- **Security:** API keys must never be written to logs, displayed in the TUI, or persisted to disk in plaintext outside of environment variables.
- **Scalability:** The tool list must render smoothly with 500+ CLI binaries using virtual scrolling.
- **Availability:** The app must run fully offline except for AI generation features, which degrade gracefully when the network or API is unavailable.
- **Cross-platform:** Must compile and run on macOS (ARM + Intel), Linux (x86_64, ARM), and Windows (x86_64).
- **MSRV:** Minimum Supported Rust Version is 1.86.0 (required by ratatui v0.30).

## 8. Constraints & Assumptions

- Rust stable toolchain 1.86.0+ is required to build the project.
- Users must have at least one AI provider API key available as an environment variable.
- The terminal must support 256 colors or true color for the yellow theme to render correctly.
- CLI tool discovery is limited to binaries found in `$PATH` — no deep package manager introspection (Homebrew, apt, etc.) in v1.
- Generated skills are markdown files following the convention expected by Claude Code and similar tools.

## 9. Success Metrics / KPIs

| Metric | Target |
|--------|--------|
| End-to-end time (select tool to install skill) | < 30 seconds |
| App startup time | < 200ms |
| Skills generated per session (average) | >= 3 |
| GitHub stars within 3 months | 500+ |
| crates.io downloads within first month | 1,000+ |
| Issue-to-PR resolution time | < 7 days |

## 10. Design & UX Direction

**UI Style:** Terminal-native, dark background with bright yellow and amber accents. Inspired by retro terminal aesthetics combined with modern developer tooling density.

**Color Palette:**

| Role | Color Name | Hex |
|------|-----------|-----|
| Primary / Borders | Gold Yellow | `#FFD700` |
| Active / Selected | Bright Yellow | `#FFFF00` |
| Accent / Buttons | Amber | `#FFA500` |
| Background | Terminal Dark | `#1C1C1C` |
| Body Text | Light Gray | `#E0E0E0` |
| Installed Indicator | Green | `#00FF7F` |

**Design Principles:**
- Keyboard-first navigation — every action reachable without a mouse
- High information density — tool list, input, and output visible simultaneously
- Immediate feedback — streaming output, visible selection state, and status indicators

**TUI Layout Mockup:**
```
+- SkillForge CLI v1.0 ------------------------------------------------------+
+---------------------+------------------------------------------------------+
| CLI Tools           | Requirement                                          |
| ------------------- | ---------------------------------------------------- |
| / Filter...         | > Generate a skill that helps with                   |
|                     |   smart git commits with AI suggestions              |
| [x] cargo           |                                                      |
| [ ] docker     (v)  +------------------------------------------------------+
| [x] git             | Generated Skill Output                               |
| [ ] gh              | ---------------------------------------------------- |
| [ ] helm       (v)  | # git: AI Commit Assistant                           |
| [x] kubectl         | ## Description                                       |
| [ ] npm             | Helps craft conventional commit messages...          |
| [ ] terraform  (v)  |                              [streaming...]          |
|                     |                                                      |
| [Generate Skills]   |  [i] Install  [c] Copy  [r] Regenerate               |
+---------------------+------------------------------------------------------+
 Tab: Switch Panel  Space: Toggle  Enter: Generate  q: Quit  ?: Help
```

## 11. Out of Scope

- Web or Electron desktop GUI version
- Multi-user or team skill collaboration
- Skill marketplace or public skill repository
- Direct integration with IDE plugins (Cursor, VS Code, Zed)
- AI model fine-tuning or training
- Windows Terminal-specific enhancements beyond standard ANSI support
