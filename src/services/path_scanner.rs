use std::path::PathBuf;

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

pub struct PathScanner {
    skills_dir: PathBuf,
}

impl PathScanner {
    pub fn new(skills_dir: PathBuf) -> Self {
        Self { skills_dir }
    }

    pub async fn scan(&self) -> Vec<ToolEntry> {
        let skills_dir = self.skills_dir.clone();
        tokio::task::spawn_blocking(move || {
            CURATED_TOOLS
                .iter()
                .map(|&name| {
                    let skill_path = skills_dir.join(format!("{}.md", name));
                    ToolEntry {
                        name: name.to_string(),
                        path: PathBuf::from(name), // placeholder — not used for curated list
                        has_skill: skill_path.exists(),
                    }
                })
                .collect()
        })
        .await
        .unwrap_or_default()
    }
}
