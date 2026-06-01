use crate::core::config::AppConfig;
use crate::core::library::SkillLibrary;
use crate::core::linker::Linker;
use crate::core::models::*;
use std::collections::HashMap;
use std::path::Path;

pub struct ConflictDetector;

impl ConflictDetector {
    pub fn detect_name_conflicts(skills: &[SkillWithStatus]) -> Vec<String> {
        let mut name_counts: HashMap<String, usize> = HashMap::new();
        for skill in skills {
            *name_counts.entry(skill.skill.name.clone()).or_insert(0) += 1;
        }
        name_counts
            .into_iter()
            .filter(|(_, count)| *count > 1)
            .map(|(name, _)| name)
            .collect()
    }

    pub fn check_directory_conflict(tool_dir: &Path, skill_name: &str) -> Option<String> {
        let target = tool_dir.join(skill_name);
        if target.exists() && !target.is_symlink() {
            Some(format!(
                "A real directory '{}' already exists at '{}'. Remove it manually first.",
                skill_name,
                target.display()
            ))
        } else {
            None
        }
    }

    pub fn check_link_integrity(
        skill: &Skill,
        tools: &[Tool],
        library: &SkillLibrary,
    ) -> HashMap<String, SkillToolStatus> {
        let skill_path = library.skill_path(&skill.name);
        let mut statuses = HashMap::new();

        for tool in tools {
            if !tool.enabled {
                continue;
            }
            // Resolve `~` so a tool path like `~/.opencode/skills` matches the
            // symlink that was actually created by `Linker::link` (which uses
            // `expanded_path()` on the write side).
            let tool_dir = tool.expanded_path();
            let status = Linker::check_status(&skill_path, &tool_dir, &skill.name);
            statuses.insert(tool.id.clone(), status);
        }

        statuses
    }

    pub fn analyze_all_conflicts(
        skills: &[SkillWithStatus],
        config: &AppConfig,
        library: &SkillLibrary,
    ) -> HashMap<String, Vec<String>> {
        let mut conflicts: HashMap<String, Vec<String>> = HashMap::new();

        for skill in skills {
            let _skill_path = library.skill_path(&skill.skill.name);
            for tool in &config.tools {
                if !tool.enabled {
                    continue;
                }
                // Match the write-side behavior: resolve `~` via expanded_path.
                let tool_dir = tool.expanded_path();
                if let Some(msg) = Self::check_directory_conflict(&tool_dir, &skill.skill.name) {
                    conflicts
                        .entry(skill.skill.name.clone())
                        .or_default()
                        .push(format!("{}: {}", tool.name, msg));
                }
            }
        }

        conflicts
    }
}
