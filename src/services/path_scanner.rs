use std::path::PathBuf;

use crate::config::get_tool_skill_path;
use crate::types::ToolEntry;

/// The five curated AI coding CLI tools SkillForge targets.
/// Discovery is explicit — not a full PATH scan — so users get a focused,
/// high-value list rather than hundreds of system binaries.
const CURATED_TOOLS: &[&str] = &[
    "codex",
    "claude-code",
    "gemini-cli",
    "opencode",
    "copilot-cli",
];

pub struct PathScanner;

impl PathScanner {
    pub fn new() -> Self {
        Self
    }

    pub async fn scan(&self) -> Vec<ToolEntry> {
        tokio::task::spawn_blocking(move || {
            CURATED_TOOLS
                .iter()
                .map(|&name| {
                    // Default skill uses the tool name as the skill name.
                    let skill_path = get_tool_skill_path(name, name);
                    ToolEntry {
                        name: name.to_string(),
                        path: PathBuf::from(name), // placeholder — not used for curated list
                        has_skill: skill_path.exists(),
                        skill_path,
                    }
                })
                .collect()
        })
        .await
        .unwrap_or_default()
    }
}
