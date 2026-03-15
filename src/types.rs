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
