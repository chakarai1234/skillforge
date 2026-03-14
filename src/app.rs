use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;
use tokio::sync::mpsc;

use crate::config::{get_skills_dir, Config};
use crate::providers::{build_provider, fetch_provider_models, AIProvider};
use crate::services::path_scanner::PathScanner;
use crate::services::skill_store::SkillStore;
use crate::types::{AppState, AppTab, Focus, ProviderEntry, StreamToken, ToolEntry};

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub config: Config,
    pub provider: Arc<dyn AIProvider>,
    pub skill_store: SkillStore,

    // ── Global UI ────────────────────────────────────────────────────────────
    pub active_tab: AppTab,
    pub focus: Focus,
    pub show_help: bool,
    pub status_message: Option<(String, bool)>, // (text, is_error)

    // ── Skills tab ───────────────────────────────────────────────────────────
    pub state: AppState,
    pub tools: Vec<ToolEntry>,
    pub filter: String,
    pub filtered_indices: Vec<usize>,
    pub selected_tools: HashSet<String>,
    pub list_index: usize,
    pub list_state: ListState,

    pub skill_name: String,        // user-specified name for the skill file
    pub skill_name_cursor: usize,
    pub requirement: String,
    pub cursor_pos: usize,
    pub output: String,
    pub output_scroll: u16,
    pub current_tool: Option<String>,

    // ── Providers tab ────────────────────────────────────────────────────────
    pub providers: Vec<ProviderEntry>,
    pub active_provider_idx: usize,
    pub editing_provider_idx: usize,
    pub key_cursor: usize,
    pub model_cursor: usize,
}

impl App {
    pub async fn new(custom_config_path: Option<std::path::PathBuf>) -> Result<Self> {
        // Load saved preferences (provider name + model — no API keys)
        let config = Config::load(custom_config_path);

        let mut providers = init_providers();

        // Apply saved provider + model from config
        let active_provider_idx = {
            let saved_name = &config.provider.name;
            let saved_model = &config.provider.model;
            let idx = providers
                .iter()
                .position(|p| p.id == saved_name.as_str())
                .unwrap_or_else(|| {
                    providers.iter().position(|p| p.is_configured()).unwrap_or(0)
                });
            // Restore saved model for the configured provider
            providers[idx].model = saved_model.clone();
            idx
        };

        let provider: Arc<dyn AIProvider> =
            Arc::from(build_provider(&providers[active_provider_idx]));

        let skills_dir = get_skills_dir();
        let skill_store = SkillStore::new(skills_dir.clone())?;
        let tools = PathScanner::new(skills_dir).scan().await;

        let filtered_indices: Vec<usize> = (0..tools.len()).collect();
        let mut list_state = ListState::default();
        if !tools.is_empty() {
            list_state.select(Some(0));
        }

        Ok(App {
            config,
            provider,
            skill_store,

            active_tab: AppTab::Skills,
            focus: Focus::ToolList,
            show_help: false,
            status_message: None,

            state: AppState::Idle,
            tools,
            filter: String::new(),
            filtered_indices,
            selected_tools: HashSet::new(),
            list_index: 0,
            list_state,

            skill_name: String::new(),
            skill_name_cursor: 0,
            requirement: String::new(),
            cursor_pos: 0,
            output: String::new(),
            output_scroll: 0,
            current_tool: None,

            providers,
            active_provider_idx,
            editing_provider_idx: active_provider_idx,
            key_cursor: 0,
            model_cursor: 0,
        })
    }

    // ── Provider rebuild + config save ────────────────────────────────────────

    fn rebuild_provider(&mut self) {
        self.provider = Arc::from(build_provider(&self.providers[self.active_provider_idx]));
        self.save_config();
    }

    fn save_config(&mut self) {
        let name = self.providers[self.active_provider_idx].id.to_string();
        let model = self.providers[self.active_provider_idx].model.clone();
        if let Err(e) = self.config.update_and_save(&name, &model) {
            tracing::warn!("Failed to save config: {}", e);
        }
    }

    // ── Main key dispatcher ───────────────────────────────────────────────────

    pub async fn handle_key(
        &mut self,
        key: KeyEvent,
        ai_tx: &mpsc::Sender<StreamToken>,
        models_tx: &mpsc::Sender<(String, Vec<String>)>,
    ) -> Result<bool> {
        let in_text = matches!(
            self.focus,
            Focus::RequirementInput
                | Focus::SearchBar
                | Focus::ApiKeyField
                | Focus::SkillName
        );

        // ── Global shortcuts ────────────────────────────────────────────────
        match key.code {
            KeyCode::Char('q') if !in_text => return Ok(true),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Ok(true)
            }
            KeyCode::Char('?') if !in_text => {
                self.show_help = !self.show_help;
                return Ok(false);
            }
            KeyCode::Esc => {
                if self.show_help {
                    self.show_help = false;
                    return Ok(false);
                }
                return Ok(self.handle_esc());
            }
            KeyCode::Char('1') if !in_text => {
                self.switch_tab(AppTab::Skills);
                return Ok(false);
            }
            KeyCode::Char('2') if !in_text => {
                self.switch_tab(AppTab::Providers);
                // Trigger model fetch for the currently editing provider
                self.trigger_model_fetch(self.editing_provider_idx, models_tx);
                return Ok(false);
            }
            KeyCode::Tab => {
                self.cycle_focus(models_tx);
                return Ok(false);
            }
            KeyCode::BackTab => {
                self.cycle_focus_back();
                return Ok(false);
            }
            _ => {}
        }

        match self.active_tab {
            AppTab::Skills => self.handle_skills_key(key, ai_tx).await,
            AppTab::Providers => self.handle_providers_key(key, models_tx),
        }
    }

    // ── Escape ────────────────────────────────────────────────────────────────

    fn handle_esc(&mut self) -> bool {
        match self.focus {
            Focus::SearchBar => {
                self.filter.clear();
                self.update_filter();
                self.focus = Focus::ToolList;
            }
            Focus::SkillName => {
                self.skill_name.clear();
                self.skill_name_cursor = 0;
            }
            Focus::RequirementInput => {
                self.requirement.clear();
                self.cursor_pos = 0;
            }
            Focus::ApiKeyField | Focus::ModelField => {
                self.focus = Focus::ProviderList;
            }
            _ => {}
        }
        false
    }

    // ── Tab switching ─────────────────────────────────────────────────────────

    fn switch_tab(&mut self, tab: AppTab) {
        self.active_tab = tab;
        self.focus = match self.active_tab {
            AppTab::Skills => Focus::ToolList,
            AppTab::Providers => Focus::ProviderList,
        };
        self.show_help = false;
    }

    // ── Focus cycling ─────────────────────────────────────────────────────────

    fn cycle_focus(&mut self, models_tx: &mpsc::Sender<(String, Vec<String>)>) {
        self.focus = match (&self.active_tab, &self.focus) {
            // Skills: List → SkillName → Requirement → Output → List
            (AppTab::Skills, Focus::ToolList | Focus::SearchBar) => Focus::SkillName,
            (AppTab::Skills, Focus::SkillName) => Focus::RequirementInput,
            (AppTab::Skills, Focus::RequirementInput) => Focus::SkillOutput,
            (AppTab::Skills, Focus::SkillOutput) => Focus::ToolList,
            // Providers: List → ApiKey → Model → List
            (AppTab::Providers, Focus::ProviderList) => {
                self.key_cursor =
                    self.providers[self.editing_provider_idx].api_key.len();
                Focus::ApiKeyField
            }
            (AppTab::Providers, Focus::ApiKeyField) => {
                // Trigger model fetch when moving to the model field
                self.trigger_model_fetch(self.editing_provider_idx, models_tx);
                self.model_cursor =
                    self.providers[self.editing_provider_idx].model.len();
                Focus::ModelField
            }
            (AppTab::Providers, Focus::ModelField) => Focus::ProviderList,
            _ => self.focus.clone(),
        };
    }

    fn cycle_focus_back(&mut self) {
        self.focus = match (&self.active_tab, &self.focus) {
            (AppTab::Skills, Focus::ToolList | Focus::SearchBar) => Focus::SkillOutput,
            (AppTab::Skills, Focus::SkillName) => Focus::ToolList,
            (AppTab::Skills, Focus::RequirementInput) => Focus::SkillName,
            (AppTab::Skills, Focus::SkillOutput) => Focus::RequirementInput,
            (AppTab::Providers, Focus::ProviderList) => Focus::ModelField,
            (AppTab::Providers, Focus::ApiKeyField) => Focus::ProviderList,
            (AppTab::Providers, Focus::ModelField) => Focus::ApiKeyField,
            _ => self.focus.clone(),
        };
    }

    // ── Skills tab ────────────────────────────────────────────────────────────

    async fn handle_skills_key(
        &mut self,
        key: KeyEvent,
        ai_tx: &mpsc::Sender<StreamToken>,
    ) -> Result<bool> {
        match self.focus {
            Focus::ToolList => self.skills_tool_list_key(key, ai_tx).await,
            Focus::SearchBar => self.skills_search_key(key),
            Focus::SkillName => self.skills_name_key(key),
            Focus::RequirementInput => self.skills_requirement_key(key, ai_tx).await,
            Focus::SkillOutput => self.skills_output_key(key, ai_tx).await,
            _ => Ok(false),
        }
    }

    async fn skills_tool_list_key(
        &mut self,
        key: KeyEvent,
        ai_tx: &mpsc::Sender<StreamToken>,
    ) -> Result<bool> {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.move_list_up(),
            KeyCode::Down | KeyCode::Char('j') => self.move_list_down(),
            KeyCode::Char(' ') => self.toggle_selection(),
            KeyCode::Char('/') => self.focus = Focus::SearchBar,
            KeyCode::Enter => self.start_generation(ai_tx).await?,
            _ => {}
        }
        Ok(false)
    }

    fn skills_search_key(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Char(c) => {
                self.filter.push(c);
                self.update_filter();
            }
            KeyCode::Backspace => {
                self.filter.pop();
                self.update_filter();
            }
            KeyCode::Enter => self.focus = Focus::ToolList,
            _ => {}
        }
        Ok(false)
    }

    fn skills_name_key(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Char(c) => {
                self.skill_name.insert(self.skill_name_cursor, c);
                self.skill_name_cursor += 1;
            }
            KeyCode::Backspace => {
                if self.skill_name_cursor > 0 {
                    self.skill_name_cursor -= 1;
                    self.skill_name.remove(self.skill_name_cursor);
                }
            }
            KeyCode::Left => {
                self.skill_name_cursor = self.skill_name_cursor.saturating_sub(1);
            }
            KeyCode::Right => {
                if self.skill_name_cursor < self.skill_name.len() {
                    self.skill_name_cursor += 1;
                }
            }
            KeyCode::Home => self.skill_name_cursor = 0,
            KeyCode::End => self.skill_name_cursor = self.skill_name.len(),
            _ => {}
        }
        Ok(false)
    }

    async fn skills_requirement_key(
        &mut self,
        key: KeyEvent,
        ai_tx: &mpsc::Sender<StreamToken>,
    ) -> Result<bool> {
        match key.code {
            KeyCode::Char(c) => {
                self.requirement.insert(self.cursor_pos, c);
                self.cursor_pos += 1;
            }
            KeyCode::Backspace => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    self.requirement.remove(self.cursor_pos);
                }
            }
            KeyCode::Left => self.cursor_pos = self.cursor_pos.saturating_sub(1),
            KeyCode::Right => {
                if self.cursor_pos < self.requirement.len() {
                    self.cursor_pos += 1;
                }
            }
            KeyCode::Home => self.cursor_pos = 0,
            KeyCode::End => self.cursor_pos = self.requirement.len(),
            KeyCode::Enter => self.start_generation(ai_tx).await?,
            _ => {}
        }
        Ok(false)
    }

    async fn skills_output_key(
        &mut self,
        key: KeyEvent,
        ai_tx: &mpsc::Sender<StreamToken>,
    ) -> Result<bool> {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.output_scroll = self.output_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.output_scroll = self.output_scroll.saturating_add(1);
            }
            KeyCode::PageUp => {
                self.output_scroll = self.output_scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.output_scroll = self.output_scroll.saturating_add(10);
            }
            KeyCode::Char('i') => self.install_skill(),
            KeyCode::Char('c') => self.copy_to_clipboard(),
            KeyCode::Char('r') => self.start_generation(ai_tx).await?,
            _ => {}
        }
        Ok(false)
    }

    // ── Providers tab ─────────────────────────────────────────────────────────

    fn handle_providers_key(
        &mut self,
        key: KeyEvent,
        models_tx: &mpsc::Sender<(String, Vec<String>)>,
    ) -> Result<bool> {
        match self.focus {
            Focus::ProviderList => self.providers_list_key(key, models_tx),
            Focus::ApiKeyField => self.providers_api_key_field(key),
            Focus::ModelField => self.providers_model_field(key),
            _ => Ok(false),
        }
    }

    fn providers_list_key(
        &mut self,
        key: KeyEvent,
        models_tx: &mpsc::Sender<(String, Vec<String>)>,
    ) -> Result<bool> {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.editing_provider_idx > 0 {
                    self.editing_provider_idx -= 1;
                    self.trigger_model_fetch(self.editing_provider_idx, models_tx);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.editing_provider_idx + 1 < self.providers.len() {
                    self.editing_provider_idx += 1;
                    self.trigger_model_fetch(self.editing_provider_idx, models_tx);
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.active_provider_idx = self.editing_provider_idx;
                self.rebuild_provider();
                let name = self.providers[self.active_provider_idx].display;
                self.status_message = Some((format!("Activated: {}", name), false));
                // Jump to API key config
                self.focus = Focus::ApiKeyField;
                self.key_cursor =
                    self.providers[self.editing_provider_idx].api_key.len();
            }
            _ => {}
        }
        Ok(false)
    }

    fn providers_api_key_field(&mut self, key: KeyEvent) -> Result<bool> {
        // Ctrl+H must be checked before the generic Char(c) arm
        if key.code == KeyCode::Char('h') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.providers[self.editing_provider_idx].show_key ^= true;
            return Ok(false);
        }

        let entry = &mut self.providers[self.editing_provider_idx];
        match key.code {
            KeyCode::Char(c) => {
                entry.api_key.insert(self.key_cursor, c);
                self.key_cursor += 1;
            }
            KeyCode::Backspace => {
                if self.key_cursor > 0 {
                    self.key_cursor -= 1;
                    entry.api_key.remove(self.key_cursor);
                }
            }
            KeyCode::Delete => {
                if self.key_cursor < entry.api_key.len() {
                    entry.api_key.remove(self.key_cursor);
                }
            }
            KeyCode::Left => self.key_cursor = self.key_cursor.saturating_sub(1),
            KeyCode::Right => {
                let len = self.providers[self.editing_provider_idx].api_key.len();
                if self.key_cursor < len {
                    self.key_cursor += 1;
                }
            }
            KeyCode::Home => self.key_cursor = 0,
            KeyCode::End => {
                self.key_cursor =
                    self.providers[self.editing_provider_idx].api_key.len();
            }
            KeyCode::Enter => {
                self.active_provider_idx = self.editing_provider_idx;
                self.rebuild_provider();
                let name = self.providers[self.active_provider_idx].display;
                self.status_message =
                    Some((format!("Saved & activated: {}", name), false));
            }
            _ => {}
        }
        Ok(false)
    }

    fn providers_model_field(&mut self, key: KeyEvent) -> Result<bool> {
        let entry = &self.providers[self.editing_provider_idx];
        let has_models = !entry.available_models.is_empty();

        if has_models {
            // ── Left/Right navigate through fetched model list ────────────────
            match key.code {
                KeyCode::Left => {
                    {
                        let entry = &mut self.providers[self.editing_provider_idx];
                        if entry.model_idx > 0 {
                            entry.model_idx -= 1;
                            entry.model = entry.available_models[entry.model_idx].clone();
                        }
                    }
                    if self.editing_provider_idx == self.active_provider_idx {
                        self.rebuild_provider();
                    }
                }
                KeyCode::Right => {
                    {
                        let entry = &mut self.providers[self.editing_provider_idx];
                        let max = entry.available_models.len().saturating_sub(1);
                        if entry.model_idx < max {
                            entry.model_idx += 1;
                            entry.model = entry.available_models[entry.model_idx].clone();
                        }
                    }
                    if self.editing_provider_idx == self.active_provider_idx {
                        self.rebuild_provider();
                    }
                }
                KeyCode::Enter => {
                    self.active_provider_idx = self.editing_provider_idx;
                    self.rebuild_provider();
                    let name = self.providers[self.active_provider_idx].display;
                    self.status_message =
                        Some((format!("Saved & activated: {}", name), false));
                }
                _ => {}
            }
        } else {
            // ── Fallback: plain text editing when models not yet loaded ────────
            let entry = &mut self.providers[self.editing_provider_idx];
            match key.code {
                KeyCode::Char(c) => {
                    entry.model.insert(self.model_cursor, c);
                    self.model_cursor += 1;
                }
                KeyCode::Backspace => {
                    if self.model_cursor > 0 {
                        self.model_cursor -= 1;
                        entry.model.remove(self.model_cursor);
                    }
                }
                KeyCode::Delete => {
                    if self.model_cursor < entry.model.len() {
                        entry.model.remove(self.model_cursor);
                    }
                }
                KeyCode::Left => {
                    self.model_cursor = self.model_cursor.saturating_sub(1);
                }
                KeyCode::Right => {
                    let len = self.providers[self.editing_provider_idx].model.len();
                    if self.model_cursor < len {
                        self.model_cursor += 1;
                    }
                }
                KeyCode::Home => self.model_cursor = 0,
                KeyCode::End => {
                    self.model_cursor =
                        self.providers[self.editing_provider_idx].model.len();
                }
                KeyCode::Enter => {
                    self.active_provider_idx = self.editing_provider_idx;
                    self.rebuild_provider();
                    let name = self.providers[self.active_provider_idx].display;
                    self.status_message =
                        Some((format!("Saved & activated: {}", name), false));
                }
                _ => {}
            }
        }
        Ok(false)
    }

    // ── Model fetch trigger ───────────────────────────────────────────────────

    pub fn trigger_model_fetch(
        &mut self,
        idx: usize,
        models_tx: &mpsc::Sender<(String, Vec<String>)>,
    ) {
        let entry = &mut self.providers[idx];
        if entry.api_key.is_empty() || entry.models_loading {
            return;
        }
        entry.models_loading = true;

        let provider_id = entry.id.to_string();
        let api_key = entry.api_key.clone();
        let tx = models_tx.clone();

        tokio::spawn(async move {
            let models = fetch_provider_models(&provider_id, &api_key).await;
            let _ = tx.send((provider_id, models)).await;
        });
    }

    pub fn handle_models_loaded(&mut self, provider_id: String, models: Vec<String>) {
        if let Some(entry) = self.providers.iter_mut().find(|p| p.id == provider_id) {
            entry.models_loading = false;
            entry.available_models = models;
            entry.sync_model_idx();
        }
    }

    // ── Generation ────────────────────────────────────────────────────────────

    async fn start_generation(&mut self, ai_tx: &mpsc::Sender<StreamToken>) -> Result<()> {
        if self.requirement.trim().is_empty() {
            self.status_message = Some(("Enter a requirement first.".to_string(), true));
            return Ok(());
        }

        let tool_name = match self.filtered_indices.get(self.list_index) {
            Some(&idx) => self.tools[idx].name.clone(),
            None => {
                self.status_message = Some(("No tool selected.".to_string(), true));
                return Ok(());
            }
        };

        self.current_tool = Some(tool_name.clone());
        self.output.clear();
        self.output_scroll = 0;
        self.state = AppState::Generating;
        self.status_message = None;
        self.focus = Focus::SkillOutput;

        let provider = Arc::clone(&self.provider);
        let requirement = self.requirement.clone();
        let tx = ai_tx.clone();

        tokio::spawn(async move {
            if let Err(e) = provider.generate_skill(&tool_name, &requirement, tx.clone()).await {
                let _ = tx.send(StreamToken::Error(e.to_string())).await;
            }
        });

        Ok(())
    }

    pub fn handle_stream_token(&mut self, token: StreamToken) {
        match token {
            StreamToken::Token(text) => {
                self.output.push_str(&text);
                let lines = self.output.lines().count() as u16;
                self.output_scroll = lines.saturating_sub(20);
            }
            StreamToken::Done => {
                self.state = AppState::Ready;
                self.output_scroll = 0;
                self.status_message =
                    Some(("[i] Install   [c] Copy   [r] Regenerate".to_string(), false));
            }
            StreamToken::Error(err) => {
                self.state = AppState::Error(err.clone());
                self.output = err.clone();
                self.status_message = Some((format!("Error: {}", err), true));
            }
        }
    }

    // ── Install & clipboard ───────────────────────────────────────────────────

    fn install_skill(&mut self) {
        if self.output.is_empty() {
            self.status_message = Some(("Nothing to install.".to_string(), true));
            return;
        }
        // Prefer user-supplied skill name; fall back to tool name
        let file_key = if self.skill_name.trim().is_empty() {
            self.current_tool.clone().unwrap_or_else(|| "skill".to_string())
        } else {
            self.skill_name.trim().to_string()
        };

        match self.skill_store.install(&file_key, &self.output) {
            Ok(path) => {
                // Mark has_skill on matching tool
                if let Some(tool_name) = &self.current_tool {
                    for entry in &mut self.tools {
                        if &entry.name == tool_name {
                            entry.has_skill = true;
                        }
                    }
                }
                self.status_message =
                    Some((format!("Installed → {}", path.display()), false));
            }
            Err(e) => {
                self.status_message = Some((format!("Install failed: {}", e), true));
            }
        }
    }

    fn copy_to_clipboard(&mut self) {
        if self.output.is_empty() {
            self.status_message = Some(("Nothing to copy.".to_string(), true));
            return;
        }
        match arboard::Clipboard::new() {
            Ok(mut cb) => match cb.set_text(&self.output) {
                Ok(_) => {
                    self.status_message = Some(("Copied to clipboard!".to_string(), false));
                }
                Err(e) => {
                    self.status_message = Some((format!("Copy failed: {}", e), true));
                }
            },
            Err(e) => {
                self.status_message = Some((format!("Clipboard: {}", e), true));
            }
        }
    }

    // ── Tool list helpers ─────────────────────────────────────────────────────

    fn move_list_up(&mut self) {
        if self.filtered_indices.is_empty() { return; }
        self.list_index = self.list_index.saturating_sub(1);
        self.list_state.select(Some(self.list_index));
    }

    fn move_list_down(&mut self) {
        if self.filtered_indices.is_empty() { return; }
        let max = self.filtered_indices.len() - 1;
        self.list_index = (self.list_index + 1).min(max);
        self.list_state.select(Some(self.list_index));
    }

    fn toggle_selection(&mut self) {
        if let Some(&idx) = self.filtered_indices.get(self.list_index) {
            let name = self.tools[idx].name.clone();
            if !self.selected_tools.remove(&name) {
                self.selected_tools.insert(name);
            }
        }
    }

    pub fn update_filter(&mut self) {
        let filter = self.filter.to_lowercase();
        self.filtered_indices = self
            .tools
            .iter()
            .enumerate()
            .filter(|(_, t)| filter.is_empty() || t.name.to_lowercase().contains(&filter))
            .map(|(i, _)| i)
            .collect();

        if !self.filtered_indices.is_empty() {
            self.list_index = self.list_index.min(self.filtered_indices.len() - 1);
            self.list_state.select(Some(self.list_index));
        } else {
            self.list_index = 0;
            self.list_state.select(None);
        }
    }
}

// ── Provider initialisation ───────────────────────────────────────────────────

fn init_providers() -> Vec<ProviderEntry> {
    vec![
        ProviderEntry::new("claude", "Claude (Anthropic)", "ANTHROPIC_API_KEY", "claude-sonnet-4-6"),
        ProviderEntry::new("openai", "OpenAI", "OPENAI_API_KEY", "gpt-4o"),
        ProviderEntry::new("gemini", "Google Gemini", "GEMINI_API_KEY", "gemini-2.0-flash"),
        ProviderEntry::new("openrouter", "OpenRouter", "OPENROUTER_API_KEY", "openai/gpt-4o"),
    ]
}
