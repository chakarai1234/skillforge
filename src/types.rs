use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct ToolEntry {
    pub name: String,
    #[allow(dead_code)]
    pub path: PathBuf,
    pub has_skill: bool,
    /// Full path where the default skill file lives (or would live).
    pub skill_path: PathBuf,
}

#[derive(Debug, Clone)]
pub enum StreamToken {
    Token(String),
    Done,
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Idle,
    Generating,
    Ready,
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppTab {
    Skills,
    Providers,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Focus {
    // ── Skills tab ────────────────────────────────────────────
    ToolList,
    SearchBar,
    SkillName, // new: skill name input at top of right panel
    RequirementInput,
    SkillOutput,
    // ── Providers tab ─────────────────────────────────────────
    ProviderList,
    ApiKeyField,
    ModelField, // left/right arrow to pick model from fetched list
}

// ── Provider entry ────────────────────────────────────────────────────────────

pub struct ProviderEntry {
    pub id: &'static str,
    pub display: &'static str,
    pub env_var: &'static str,
    #[allow(dead_code)]
    pub default_model: &'static str,
    /// Editable API key (pre-loaded from env var, never persisted)
    pub api_key: String,
    /// Currently selected model (persisted to config.toml)
    pub model: String,
    /// Toggle to show masked/plain API key
    pub show_key: bool,
    /// Models fetched from the provider API
    pub available_models: Vec<String>,
    /// Index into available_models
    pub model_idx: usize,
    /// True while a model-list request is in flight
    pub models_loading: bool,
}

impl ProviderEntry {
    pub fn new(
        id: &'static str,
        display: &'static str,
        env_var: &'static str,
        default_model: &'static str,
    ) -> Self {
        let api_key = std::env::var(env_var).unwrap_or_default();
        Self {
            id,
            display,
            env_var,
            default_model,
            api_key,
            model: default_model.to_string(),
            show_key: false,
            available_models: Vec::new(),
            model_idx: 0,
            models_loading: false,
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.api_key.is_empty()
    }

    /// Returns a display-safe version of the API key.
    pub fn display_key(&self) -> String {
        if self.api_key.is_empty() {
            return "(not set — Tab to configure)".to_string();
        }
        if self.show_key {
            self.api_key.clone()
        } else {
            let visible: String = self.api_key.chars().take(4).collect();
            let masked = "*".repeat(self.api_key.len().saturating_sub(4).min(20));
            format!("{visible}{masked}")
        }
    }

    /// Sync model_idx to match the current model string after models are loaded.
    pub fn sync_model_idx(&mut self) {
        if let Some(idx) = self.available_models.iter().position(|m| m == &self.model) {
            self.model_idx = idx;
        } else if !self.available_models.is_empty() {
            // keep model_idx at 0 but don't overwrite manually-typed model
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn make_entry() -> ProviderEntry {
        ProviderEntry {
            id: "test",
            display: "Test Provider",
            env_var: "TEST_UNUSED_VAR",
            default_model: "model-x",
            api_key: String::new(),
            model: "model-x".to_string(),
            show_key: false,
            available_models: Vec::new(),
            model_idx: 0,
            models_loading: false,
        }
    }

    // ── ProviderEntry::new ────────────────────────────────────────────────────

    #[test]
    #[allow(deprecated)]
    fn provider_entry_new_sets_fields() {
        // SAFETY: unique var name, no other test reads or sets it.
        unsafe {
            env::remove_var("SKILLFORGE_TEST_KEY_NEW");
        }
        let e = ProviderEntry::new("p", "My P", "SKILLFORGE_TEST_KEY_NEW", "m1");
        assert_eq!(e.id, "p");
        assert_eq!(e.display, "My P");
        assert_eq!(e.env_var, "SKILLFORGE_TEST_KEY_NEW");
        assert_eq!(e.default_model, "m1");
        assert_eq!(e.model, "m1");
        assert_eq!(e.api_key, "");
        assert!(!e.show_key);
        assert!(e.available_models.is_empty());
        assert_eq!(e.model_idx, 0);
        assert!(!e.models_loading);
    }

    #[test]
    #[allow(deprecated)]
    fn provider_entry_new_reads_env_key() {
        // SAFETY: tests run in a single-threaded harness and use a unique var name
        // not shared with any other test.
        unsafe {
            env::set_var("SKILLFORGE_TEST_KEY_READ", "sk-hello");
        }
        let e = ProviderEntry::new("p", "P", "SKILLFORGE_TEST_KEY_READ", "m");
        assert_eq!(e.api_key, "sk-hello");
        unsafe {
            env::remove_var("SKILLFORGE_TEST_KEY_READ");
        }
    }

    // ── is_configured ─────────────────────────────────────────────────────────

    #[test]
    fn is_configured_false_when_empty() {
        let e = make_entry();
        assert!(!e.is_configured());
    }

    #[test]
    fn is_configured_true_when_key_set() {
        let mut e = make_entry();
        e.api_key = "some-key".to_string();
        assert!(e.is_configured());
    }

    // ── display_key ───────────────────────────────────────────────────────────

    #[test]
    fn display_key_empty_returns_placeholder() {
        let e = make_entry();
        assert_eq!(e.display_key(), "(not set — Tab to configure)");
    }

    #[test]
    fn display_key_masked_starts_with_first_four_chars() {
        let mut e = make_entry();
        e.api_key = "sk-abcdefgh".to_string();
        let result = e.display_key();
        assert!(result.starts_with("sk-a"));
        assert!(result.contains('*'));
    }

    #[test]
    fn display_key_short_key_no_mask() {
        let mut e = make_entry();
        e.api_key = "abc".to_string(); // len=3, saturating_sub(4)=0 → no stars
        let result = e.display_key();
        assert_eq!(result, "abc");
    }

    #[test]
    fn display_key_show_plain_returns_full_key() {
        let mut e = make_entry();
        e.api_key = "sk-full-key".to_string();
        e.show_key = true;
        assert_eq!(e.display_key(), "sk-full-key");
    }

    #[test]
    fn display_key_mask_capped_at_20_stars() {
        let mut e = make_entry();
        e.api_key = "x".repeat(60); // very long
        let result = e.display_key();
        let star_count = result.chars().filter(|&c| c == '*').count();
        assert!(star_count <= 20);
    }

    // ── sync_model_idx ────────────────────────────────────────────────────────

    #[test]
    fn sync_model_idx_finds_correct_index() {
        let mut e = make_entry();
        e.available_models = vec!["m1".to_string(), "m2".to_string(), "m3".to_string()];
        e.model = "m2".to_string();
        e.sync_model_idx();
        assert_eq!(e.model_idx, 1);
    }

    #[test]
    fn sync_model_idx_not_found_keeps_zero() {
        let mut e = make_entry();
        e.available_models = vec!["m1".to_string(), "m2".to_string()];
        e.model = "unknown".to_string();
        e.model_idx = 0;
        e.sync_model_idx();
        assert_eq!(e.model_idx, 0);
    }

    #[test]
    fn sync_model_idx_empty_models_unchanged() {
        let mut e = make_entry();
        e.available_models = vec![];
        e.model_idx = 0;
        e.sync_model_idx();
        assert_eq!(e.model_idx, 0);
    }

    // ── AppState / AppTab / StreamToken ───────────────────────────────────────

    #[test]
    fn app_state_equality() {
        assert_eq!(AppState::Idle, AppState::Idle);
        assert_eq!(AppState::Ready, AppState::Ready);
        assert_eq!(AppState::Generating, AppState::Generating);
        assert_eq!(AppState::Error("x".into()), AppState::Error("x".into()));
        assert_ne!(AppState::Idle, AppState::Ready);
        assert_ne!(AppState::Error("a".into()), AppState::Error("b".into()));
    }

    #[test]
    fn app_tab_equality() {
        assert_eq!(AppTab::Skills, AppTab::Skills);
        assert_eq!(AppTab::Providers, AppTab::Providers);
        assert_ne!(AppTab::Skills, AppTab::Providers);
    }

    #[test]
    fn stream_token_debug_does_not_panic() {
        let _ = format!("{:?}", StreamToken::Token("hello".to_string()));
        let _ = format!("{:?}", StreamToken::Done);
        let _ = format!("{:?}", StreamToken::Error("oops".to_string()));
    }

    #[test]
    fn tool_entry_fields() {
        let e = ToolEntry {
            name: "claude-code".to_string(),
            path: std::path::PathBuf::from("claude-code"),
            has_skill: true,
            skill_path: std::path::PathBuf::from("/home/user/.claude/skills/claude-code/SKILL.md"),
        };
        assert_eq!(e.name, "claude-code");
        assert!(e.has_skill);
        assert_eq!(e.skill_path.file_name().unwrap(), "SKILL.md");
    }
}
