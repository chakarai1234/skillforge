use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::config::get_tool_skill_path;

pub struct SkillStore;

impl SkillStore {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    /// Write skill content to `~/{tool_base}/skills/{skill_name}/SKILL.md`,
    /// creating the directory hierarchy as needed.
    pub fn install(&self, tool: &str, skill_name: &str, content: &str) -> Result<PathBuf> {
        let path = get_tool_skill_path(tool, skill_name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create dir: {}", parent.display()))?;
        }
        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write skill: {}", path.display()))?;
        Ok(path)
    }
}
