use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;
use tokio::sync::mpsc;

use crate::config::Config;
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

    pub skill_name: String, // user-specified name for the skill file
    pub skill_name_cursor: usize,
    pub requirement: String,
    pub cursor_pos: usize,
    pub output: String,
    pub output_scroll: u16,
    pub current_tool: Option<String>,
    pub current_skill_name: String, // effective name used for the active generation

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
                    providers
                        .iter()
                        .position(|p| p.is_configured())
                        .unwrap_or(0)
                });
            // Restore saved model for the configured provider
            providers[idx].model = saved_model.clone();
            idx
        };

        let provider: Arc<dyn AIProvider> =
            Arc::from(build_provider(&providers[active_provider_idx]));

        let skill_store = SkillStore::new()?;
        let tools = PathScanner::new().scan().await;

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
            current_skill_name: String::new(),

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
            Focus::RequirementInput | Focus::SearchBar | Focus::ApiKeyField | Focus::SkillName
        );

        // ── Global shortcuts ────────────────────────────────────────────────
        match key.code {
            KeyCode::Char('q') if !in_text => return Ok(true),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(true),
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
                self.key_cursor = self.providers[self.editing_provider_idx].api_key.len();
                Focus::ApiKeyField
            }
            (AppTab::Providers, Focus::ApiKeyField) => {
                // Trigger model fetch when moving to the model field
                self.trigger_model_fetch(self.editing_provider_idx, models_tx);
                self.model_cursor = self.providers[self.editing_provider_idx].model.len();
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
                self.status_message = Some((format!("Activated: {name}"), false));
                // Jump to API key config
                self.focus = Focus::ApiKeyField;
                self.key_cursor = self.providers[self.editing_provider_idx].api_key.len();
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
                self.key_cursor = self.providers[self.editing_provider_idx].api_key.len();
            }
            KeyCode::Enter => {
                self.active_provider_idx = self.editing_provider_idx;
                self.rebuild_provider();
                let name = self.providers[self.active_provider_idx].display;
                self.status_message = Some((format!("Saved & activated: {name}"), false));
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
                    self.status_message = Some((format!("Saved & activated: {name}"), false));
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
                    self.model_cursor = self.providers[self.editing_provider_idx].model.len();
                }
                KeyCode::Enter => {
                    self.active_provider_idx = self.editing_provider_idx;
                    self.rebuild_provider();
                    let name = self.providers[self.active_provider_idx].display;
                    self.status_message = Some((format!("Saved & activated: {name}"), false));
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
        let skill_name = if self.skill_name.trim().is_empty() {
            tool_name.clone()
        } else {
            self.skill_name.trim().to_string()
        };
        self.current_skill_name = skill_name.clone();
        let tx = ai_tx.clone();

        tokio::spawn(async move {
            if let Err(e) = provider
                .generate_skill(&tool_name, &skill_name, &requirement, tx.clone())
                .await
            {
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
                // Trim trailing whitespace and fix the name: field so the output
                // panel shows the correct skill name immediately (before install).
                self.output =
                    fix_frontmatter_name(self.output.trim_end(), &self.current_skill_name.clone());
                self.status_message =
                    Some(("[i] Install   [c] Copy   [r] Regenerate".to_string(), false));
            }
            StreamToken::Error(err) => {
                self.state = AppState::Error(err.clone());
                self.output = err.clone();
                self.status_message = Some((format!("Error: {err}"), true));
            }
        }
    }

    // ── Install & clipboard ───────────────────────────────────────────────────

    fn install_skill(&mut self) {
        if self.output.is_empty() {
            self.status_message = Some(("Nothing to install.".to_string(), true));
            return;
        }

        // Prefer user-supplied skill name; fall back to the current tool name
        let skill_name = if self.skill_name.trim().is_empty() {
            self.current_tool
                .clone()
                .unwrap_or_else(|| "skill".to_string())
        } else {
            self.skill_name.trim().to_string()
        };

        // Install to all selected tools (if any), otherwise fall back to current_tool
        let targets: Vec<String> = if !self.selected_tools.is_empty() {
            let mut v: Vec<String> = self.selected_tools.iter().cloned().collect();
            v.sort();
            v
        } else {
            vec![self
                .current_tool
                .clone()
                .unwrap_or_else(|| "skill".to_string())]
        };

        let mut last_path = String::new();
        let mut errors: Vec<String> = Vec::new();

        // Ensure the frontmatter name: field matches the chosen skill name
        let content = fix_frontmatter_name(&self.output, &skill_name);

        for tool in &targets {
            match self.skill_store.install(tool, &skill_name, &content) {
                Ok(path) => {
                    // Mark has_skill on the matching tool entry
                    for entry in &mut self.tools {
                        if &entry.name == tool {
                            entry.has_skill = true;
                        }
                    }
                    last_path = path.display().to_string();
                }
                Err(e) => {
                    errors.push(format!("{tool}: {e}"));
                }
            }
        }

        if errors.is_empty() {
            let msg = if targets.len() == 1 {
                format!("Installed → {last_path}")
            } else {
                format!("Installed to {} tools → {last_path}", targets.len())
            };
            self.status_message = Some((msg, false));
        } else {
            self.status_message = Some((format!("Install failed: {}", errors.join(", ")), true));
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
                    self.status_message = Some((format!("Copy failed: {e}"), true));
                }
            },
            Err(e) => {
                self.status_message = Some((format!("Clipboard: {e}"), true));
            }
        }
    }

    // ── Tool list helpers ─────────────────────────────────────────────────────

    fn move_list_up(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        self.list_index = self.list_index.saturating_sub(1);
        self.list_state.select(Some(self.list_index));
    }

    fn move_list_down(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
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

// ── Post-processing helpers ───────────────────────────────────────────────────

/// Replace the `name:` field in YAML frontmatter with `skill_name`.
/// Guarantees the installed file's name matches the folder/skill name the
/// user chose, regardless of what the AI generated.
fn fix_frontmatter_name(content: &str, skill_name: &str) -> String {
    let mut in_frontmatter = false;
    let mut frontmatter_closed = false;
    content
        .lines()
        .enumerate()
        .map(|(i, line)| {
            if i == 0 && line.trim() == "---" {
                in_frontmatter = true;
                line.to_string()
            } else if in_frontmatter && !frontmatter_closed && line.trim() == "---" {
                frontmatter_closed = true;
                in_frontmatter = false;
                line.to_string()
            } else if in_frontmatter && line.starts_with("name:") {
                format!("name: {skill_name}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ── Provider initialisation ───────────────────────────────────────────────────

fn init_providers() -> Vec<ProviderEntry> {
    vec![
        ProviderEntry::new(
            "claude",
            "Claude (Anthropic)",
            "ANTHROPIC_API_KEY",
            "claude-sonnet-4-6",
        ),
        ProviderEntry::new("openai", "OpenAI", "OPENAI_API_KEY", "gpt-4o"),
        ProviderEntry::new(
            "gemini",
            "Google Gemini",
            "GEMINI_API_KEY",
            "gemini-2.0-flash",
        ),
        ProviderEntry::new(
            "openrouter",
            "OpenRouter",
            "OPENROUTER_API_KEY",
            "openai/gpt-4o",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp_cfg(suffix: &str) -> Option<PathBuf> {
        Some(std::env::temp_dir().join(format!("skillforge_app_test_{suffix}.toml")))
    }

    // ── fix_frontmatter_name ──────────────────────────────────────────────────

    #[test]
    fn fix_frontmatter_name_replaces_name_field() {
        let input = "---\nname: old-name\ndescription: test\n---\n# Body";
        let result = fix_frontmatter_name(input, "new-name");
        assert!(result.contains("name: new-name"));
        assert!(!result.contains("name: old-name"));
    }

    #[test]
    fn fix_frontmatter_name_preserves_other_fields() {
        let input = "---\nname: old\ndescription: keep this\n---\n# Body";
        let result = fix_frontmatter_name(input, "updated");
        assert!(result.contains("description: keep this"));
        assert!(result.contains("# Body"));
        assert!(result.contains("name: updated"));
    }

    #[test]
    fn fix_frontmatter_name_preserves_body_after_frontmatter() {
        let input = "---\nname: old\n---\n# Title\n\nBody text here.";
        let result = fix_frontmatter_name(input, "new");
        assert!(result.contains("# Title"));
        assert!(result.contains("Body text here."));
    }

    #[test]
    fn fix_frontmatter_name_no_frontmatter_unchanged() {
        let input = "# Just a heading\nsome body text";
        let result = fix_frontmatter_name(input, "my-skill");
        assert_eq!(result, input);
    }

    #[test]
    fn fix_frontmatter_name_unclosed_frontmatter_still_replaces() {
        let input = "---\nname: old\ndescription: test";
        let result = fix_frontmatter_name(input, "new-name");
        assert!(result.contains("name: new-name"));
        assert!(!result.contains("name: old"));
    }

    #[test]
    fn fix_frontmatter_name_does_not_replace_name_outside_frontmatter() {
        let input = "---\nname: old\n---\nname: this-should-stay";
        let result = fix_frontmatter_name(input, "replaced");
        assert!(result.contains("name: replaced"));
        assert!(result.contains("name: this-should-stay"));
    }

    // ── App initialisation ────────────────────────────────────────────────────

    #[tokio::test]
    async fn app_initial_state_is_idle() {
        let app = App::new(tmp_cfg("init_state")).await.unwrap();
        assert_eq!(app.state, AppState::Idle);
    }

    #[tokio::test]
    async fn app_initial_tab_is_skills() {
        let app = App::new(tmp_cfg("init_tab")).await.unwrap();
        assert_eq!(app.active_tab, AppTab::Skills);
    }

    #[tokio::test]
    async fn app_initial_focus_is_tool_list() {
        let app = App::new(tmp_cfg("init_focus")).await.unwrap();
        assert_eq!(app.focus, Focus::ToolList);
    }

    #[tokio::test]
    async fn app_tools_list_contains_curated_tools() {
        let app = App::new(tmp_cfg("init_tools")).await.unwrap();
        let names: Vec<&str> = app.tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"claude-code"));
        assert!(names.contains(&"codex"));
        assert!(names.contains(&"gemini-cli"));
        assert!(names.contains(&"opencode"));
        assert!(names.contains(&"copilot-cli"));
    }

    #[tokio::test]
    async fn app_filtered_indices_matches_tools_on_start() {
        let app = App::new(tmp_cfg("init_filter")).await.unwrap();
        assert_eq!(app.filtered_indices.len(), app.tools.len());
    }

    #[tokio::test]
    async fn app_has_four_providers() {
        let app = App::new(tmp_cfg("init_providers")).await.unwrap();
        assert_eq!(app.providers.len(), 4);
        let ids: Vec<&str> = app.providers.iter().map(|p| p.id).collect();
        assert!(ids.contains(&"claude"));
        assert!(ids.contains(&"openai"));
        assert!(ids.contains(&"gemini"));
        assert!(ids.contains(&"openrouter"));
    }

    // ── update_filter ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn update_filter_empty_shows_all() {
        let mut app = App::new(tmp_cfg("filter_empty")).await.unwrap();
        app.filter = String::new();
        app.update_filter();
        assert_eq!(app.filtered_indices.len(), app.tools.len());
    }

    #[tokio::test]
    async fn update_filter_narrows_by_name() {
        let mut app = App::new(tmp_cfg("filter_narrow")).await.unwrap();
        app.filter = "claude".to_string();
        app.update_filter();
        for &idx in &app.filtered_indices {
            assert!(app.tools[idx].name.contains("claude"));
        }
    }

    #[tokio::test]
    async fn update_filter_no_match_empties_indices() {
        let mut app = App::new(tmp_cfg("filter_nomatch")).await.unwrap();
        app.filter = "zzznomatch999xyz".to_string();
        app.update_filter();
        assert!(app.filtered_indices.is_empty());
        assert_eq!(app.list_index, 0);
    }

    #[tokio::test]
    async fn update_filter_is_case_insensitive() {
        let mut app = App::new(tmp_cfg("filter_case")).await.unwrap();
        app.filter = "CLAUDE".to_string();
        app.update_filter();
        for &idx in &app.filtered_indices {
            assert!(app.tools[idx].name.to_lowercase().contains("claude"));
        }
    }

    // ── move_list_up / move_list_down ─────────────────────────────────────────

    #[tokio::test]
    async fn move_list_down_increments_index() {
        let mut app = App::new(tmp_cfg("nav_down")).await.unwrap();
        assert!(app.tools.len() > 1);
        app.list_index = 0;
        app.move_list_down();
        assert_eq!(app.list_index, 1);
    }

    #[tokio::test]
    async fn move_list_up_decrements_index() {
        let mut app = App::new(tmp_cfg("nav_up")).await.unwrap();
        app.list_index = 1;
        app.move_list_up();
        assert_eq!(app.list_index, 0);
    }

    #[tokio::test]
    async fn move_list_up_does_not_underflow() {
        let mut app = App::new(tmp_cfg("nav_up_clamp")).await.unwrap();
        app.list_index = 0;
        app.move_list_up();
        assert_eq!(app.list_index, 0);
    }

    #[tokio::test]
    async fn move_list_down_does_not_exceed_max() {
        let mut app = App::new(tmp_cfg("nav_down_clamp")).await.unwrap();
        let max = app.filtered_indices.len() - 1;
        app.list_index = max;
        app.move_list_down();
        assert_eq!(app.list_index, max);
    }

    // ── toggle_selection ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn toggle_selection_adds_tool_name() {
        let mut app = App::new(tmp_cfg("toggle_add")).await.unwrap();
        app.list_index = 0;
        let name = app.tools[app.filtered_indices[0]].name.clone();
        app.toggle_selection();
        assert!(app.selected_tools.contains(&name));
    }

    #[tokio::test]
    async fn toggle_selection_removes_when_already_selected() {
        let mut app = App::new(tmp_cfg("toggle_remove")).await.unwrap();
        app.list_index = 0;
        let name = app.tools[app.filtered_indices[0]].name.clone();
        app.toggle_selection(); // add
        app.toggle_selection(); // remove
        assert!(!app.selected_tools.contains(&name));
    }

    // ── handle_stream_token ───────────────────────────────────────────────────

    #[tokio::test]
    async fn handle_stream_token_appends_text_to_output() {
        let mut app = App::new(tmp_cfg("stream_append")).await.unwrap();
        app.handle_stream_token(StreamToken::Token("hello".to_string()));
        app.handle_stream_token(StreamToken::Token(" world".to_string()));
        assert_eq!(app.output, "hello world");
    }

    #[tokio::test]
    async fn handle_stream_token_done_sets_ready_state() {
        let mut app = App::new(tmp_cfg("stream_done")).await.unwrap();
        app.state = AppState::Generating;
        app.handle_stream_token(StreamToken::Done);
        assert_eq!(app.state, AppState::Ready);
    }

    #[tokio::test]
    async fn handle_stream_token_done_sets_status_message() {
        let mut app = App::new(tmp_cfg("stream_done_msg")).await.unwrap();
        app.handle_stream_token(StreamToken::Done);
        let (msg, is_err) = app.status_message.unwrap();
        assert!(!is_err);
        assert!(msg.contains("Install") || msg.contains("Copy") || msg.contains("Regenerate"));
    }

    #[tokio::test]
    async fn handle_stream_token_error_sets_error_state() {
        let mut app = App::new(tmp_cfg("stream_error")).await.unwrap();
        app.handle_stream_token(StreamToken::Error("something failed".to_string()));
        assert!(matches!(app.state, AppState::Error(_)));
    }

    #[tokio::test]
    async fn handle_stream_token_error_sets_error_output() {
        let mut app = App::new(tmp_cfg("stream_error_out")).await.unwrap();
        app.handle_stream_token(StreamToken::Error("boom".to_string()));
        assert_eq!(app.output, "boom");
    }

    #[tokio::test]
    async fn handle_stream_token_error_sets_status_message_as_error() {
        let mut app = App::new(tmp_cfg("stream_error_msg")).await.unwrap();
        app.handle_stream_token(StreamToken::Error("fail".to_string()));
        let (_, is_err) = app.status_message.unwrap();
        assert!(is_err);
    }

    // ── handle_models_loaded ──────────────────────────────────────────────────

    #[tokio::test]
    async fn handle_models_loaded_updates_available_models() {
        let mut app = App::new(tmp_cfg("models_loaded")).await.unwrap();
        let id = app.providers[0].id.to_string();
        app.handle_models_loaded(id.clone(), vec!["m-a".to_string(), "m-b".to_string()]);
        let p = app.providers.iter().find(|p| p.id == id).unwrap();
        assert_eq!(p.available_models, vec!["m-a", "m-b"]);
    }

    #[tokio::test]
    async fn handle_models_loaded_clears_loading_flag() {
        let mut app = App::new(tmp_cfg("models_loading")).await.unwrap();
        let id = app.providers[0].id.to_string();
        app.providers[0].models_loading = true;
        app.handle_models_loaded(id.clone(), vec![]);
        let p = app.providers.iter().find(|p| p.id == id).unwrap();
        assert!(!p.models_loading);
    }

    #[tokio::test]
    async fn handle_models_loaded_unknown_id_is_noop() {
        let mut app = App::new(tmp_cfg("models_unknown")).await.unwrap();
        // Should not panic
        app.handle_models_loaded("does-not-exist".to_string(), vec!["m".to_string()]);
    }
}
