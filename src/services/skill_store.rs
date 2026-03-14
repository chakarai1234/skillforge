use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub struct SkillStore {
    pub skills_dir: PathBuf,
}

impl SkillStore {
    pub fn new(skills_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&skills_dir)
            .with_context(|| format!("Failed to create skills dir: {}", skills_dir.display()))?;
        Ok(Self { skills_dir })
    }

    pub fn install(&self, tool: &str, content: &str) -> Result<PathBuf> {
        let path = self.skill_path(tool);
        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write skill: {}", path.display()))?;
        Ok(path)
    }

    #[allow(dead_code)]
    pub fn exists(&self, tool: &str) -> bool {
        self.skill_path(tool).exists()
    }

    #[allow(dead_code)]
    pub fn load(&self, tool: &str) -> Result<String> {
        let path = self.skill_path(tool);
        std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read skill: {}", path.display()))
    }

    fn skill_path(&self, tool: &str) -> PathBuf {
        self.skills_dir.join(format!("{}.md", tool))
    }

    #[allow(dead_code)]
    pub fn skills_dir(&self) -> &Path {
        &self.skills_dir
    }
}
