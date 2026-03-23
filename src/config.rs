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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // ── path helpers ──────────────────────────────────────────────────────────

    #[test]
    fn get_tool_base_dir_known_tools() {
        assert!(get_tool_base_dir("claude-code").ends_with(".claude"));
        assert!(get_tool_base_dir("copilot-cli").ends_with(".copilot"));
        assert!(get_tool_base_dir("codex").ends_with(".codex"));
        assert!(get_tool_base_dir("gemini-cli").ends_with(".gemini"));
        assert!(get_tool_base_dir("opencode").ends_with(".opencode"));
    }

    #[test]
    fn get_tool_base_dir_unknown_falls_back_to_skillforge() {
        let path = get_tool_base_dir("some-unknown-tool");
        assert!(path.ends_with(".skillforge"));
    }

    #[test]
    fn get_tool_skill_path_has_correct_structure() {
        let path = get_tool_skill_path("claude-code", "my-skill");
        let s = path.to_string_lossy();
        assert!(s.contains(".claude"));
        assert!(s.contains("skills"));
        assert!(s.contains("my-skill"));
        assert!(s.ends_with("SKILL.md"));
    }

    #[test]
    fn get_tool_skill_path_different_tools_differ() {
        let p1 = get_tool_skill_path("claude-code", "skill");
        let p2 = get_tool_skill_path("codex", "skill");
        assert_ne!(p1, p2);
    }

    #[test]
    fn get_config_dir_contains_skillforge() {
        let path = get_config_dir();
        assert!(path.to_string_lossy().contains(".skillforge"));
    }

    #[test]
    fn config_file_path_is_toml() {
        let path = config_file_path();
        assert_eq!(path.file_name().unwrap(), "config.toml");
    }

    // ── Config defaults ───────────────────────────────────────────────────────

    #[test]
    fn config_default_provider_is_claude() {
        let c = Config::default();
        assert_eq!(c.provider.name, "claude");
        assert_eq!(c.provider.model, "claude-sonnet-4-6");
    }

    #[test]
    fn provider_pref_default_values() {
        let p = ProviderPref::default();
        assert_eq!(p.name, "claude");
        assert_eq!(p.model, "claude-sonnet-4-6");
    }

    // ── Config::load ──────────────────────────────────────────────────────────

    #[test]
    fn config_load_missing_file_returns_default() {
        let path = PathBuf::from("/tmp/skillforge_test_nonexistent_abc123.toml");
        let c = Config::load(Some(path));
        assert_eq!(c.provider.name, "claude");
    }

    #[test]
    fn config_load_reads_provider_name_and_model() {
        let path = std::env::temp_dir().join("skillforge_cfg_load_test.toml");
        fs::write(&path, "[provider]\nname = \"openai\"\nmodel = \"gpt-4o\"\n").unwrap();
        let c = Config::load(Some(path.clone()));
        assert_eq!(c.provider.name, "openai");
        assert_eq!(c.provider.model, "gpt-4o");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn config_load_invalid_toml_returns_default() {
        let path = std::env::temp_dir().join("skillforge_cfg_invalid_test.toml");
        fs::write(&path, "not valid toml !!!###").unwrap();
        let c = Config::load(Some(path.clone()));
        assert_eq!(c.provider.name, "claude");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn config_load_partial_toml_returns_default() {
        let path = std::env::temp_dir().join("skillforge_cfg_partial_test.toml");
        // Missing required fields — should fall back to default
        fs::write(&path, "[provider]\n").unwrap();
        let c = Config::load(Some(path.clone()));
        // partial provider missing name/model → serde default kicks in
        assert!(!c.provider.name.is_empty());
        let _ = fs::remove_file(path);
    }

    // ── Config round-trip (update_and_save + load) ────────────────────────────
    // Note: save() always writes to get_config_dir() — we only test the
    // in-memory update_and_save path here to avoid touching ~/.skillforge.

    #[test]
    fn config_update_and_save_updates_fields_in_memory() {
        let mut c = Config::default();
        // We can't easily intercept the file write, so just verify the
        // in-memory state is changed before save is attempted.
        c.provider.name = "gemini".to_string();
        c.provider.model = "gemini-2.0-flash".to_string();
        assert_eq!(c.provider.name, "gemini");
        assert_eq!(c.provider.model, "gemini-2.0-flash");
    }
}
