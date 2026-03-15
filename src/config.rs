// Config for SkillForge CLI
//
// Persisted at:  ~/.skillforge/config.toml
// Skills dirs:   per-tool, e.g. ~/.claude/skills/<name>/SKILL.md
//
// NOTE: API keys are NEVER written to config — they come only from env vars.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ── Directory helpers ─────────────────────────────────────────────────────────

fn home_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
}

/// Returns the base config directory for the given tool.
/// e.g. `claude-code` → `~/.claude`, `codex` → `~/.codex`, etc.
pub fn get_tool_base_dir(tool: &str) -> PathBuf {
    let dir = match tool {
        "claude-code" => ".claude",
        "copilot-cli" => ".copilot",
        "codex" => ".codex",
        "gemini-cli" => ".gemini",
        "opencode" => ".opencode",
        _ => ".skillforge",
    };
    home_dir().join(dir)
}

/// Returns the full path where a skill file will live:
/// `~/{tool_base}/skills/{skill_name}/SKILL.md`
pub fn get_tool_skill_path(tool: &str, skill_name: &str) -> PathBuf {
    get_tool_base_dir(tool)
        .join("skills")
        .join(skill_name)
        .join("SKILL.md")
}

/// Returns `~/.skillforge` (used only for the config.toml location).
pub fn get_config_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(appdata).join(".skillforge")
    }
    #[cfg(not(target_os = "windows"))]
    {
        home_dir().join(".skillforge")
    }
}

pub fn config_file_path() -> PathBuf {
    get_config_dir().join("config.toml")
}

// ── Config struct (only non-secret preferences) ───────────────────────────────

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub provider: ProviderPref,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProviderPref {
    /// Provider ID: "claude" | "openai" | "gemini" | "openrouter"
    pub name: String,
    /// Model identifier chosen by the user
    pub model: String,
}

impl Default for ProviderPref {
    fn default() -> Self {
        ProviderPref {
            name: "claude".to_string(),
            model: "claude-sonnet-4-6".to_string(),
        }
    }
}

impl Config {
    /// Load from `~/.skillforge/config.toml` (or custom path).
    /// Falls back to defaults if the file does not exist.
    pub fn load(custom_path: Option<PathBuf>) -> Self {
        let path = custom_path.unwrap_or_else(config_file_path);
        match std::fs::read_to_string(&path) {
            Ok(content) => toml::from_str(&content).unwrap_or_default(),
            Err(_) => Config::default(),
        }
    }

    /// Persist to `~/.skillforge/config.toml`.
    /// API keys are NOT included — only provider name and model.
    pub fn save(&self) -> Result<()> {
        let dir = get_config_dir();
        std::fs::create_dir_all(&dir)?;
        let content = toml::to_string_pretty(self)?;
        let path = config_file_path();
        std::fs::write(&path, content)?;
        tracing::info!("Config saved to {}", path.display());
        Ok(())
    }

    /// Convenience: update provider + model, then save.
    pub fn update_and_save(&mut self, provider_name: &str, model: &str) -> Result<()> {
        self.provider.name = provider_name.to_string();
        self.provider.model = model.to_string();
        self.save()
    }
}
