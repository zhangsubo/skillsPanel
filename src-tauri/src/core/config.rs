use crate::core::error::AppError;
use crate::core::fs_utils;
use crate::core::models::*;
use dirs::home_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub library_path: PathBuf,
    pub tools: Vec<Tool>,
    pub sources: Vec<SourceConfig>,
    pub sync: SyncConfig,
    pub install: InstallConfig,
    pub exclude_paths: Vec<String>,
    pub rules: RulesConfig,
    pub deleted_skills: Vec<String>,
}

impl AppConfig {
    pub fn config_path() -> Result<PathBuf, AppError> {
        let home = home_dir().ok_or_else(|| AppError::Config("Cannot find home directory".into()))?;
        Ok(home.join(".skills-panel").join("skills-panel.config.json"))
    }

    pub fn default_library_path() -> PathBuf {
        home_dir()
            .map(|h| h.join(".skills-panel").join("skills"))
            .unwrap_or_else(|| PathBuf::from(".skills-panel/skills"))
    }

    pub fn load_or_create() -> Result<Self, AppError> {
        let path = Self::config_path()?;
        if path.exists() {
            Self::load(&path)
        } else {
            let config = Self::default_config();
            config.save()?;
            Ok(config)
        }
    }

    pub fn load(path: &PathBuf) -> Result<Self, AppError> {
        let content = fs::read_to_string(path)?;
        let raw: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| AppError::Config(format!("Invalid JSON: {}", e)))?;
        Self::from_json_value(raw)
    }

    fn from_json_value(raw: serde_json::Value) -> Result<Self, AppError> {
        let library_path = raw.get("library")
            .and_then(|l| l.get("path"))
            .and_then(|p| p.as_str())
            .map(|p| fs_utils::expand_tilde(p))
            .unwrap_or_else(Self::default_library_path);

        let tools: Vec<Tool> = raw.get("tools")
            .map(|t| Self::parse_tools(t))
            .unwrap_or_default();

        let sources: Vec<SourceConfig> = raw.get("sources")
            .map(|s| serde_json::from_value(s.clone()).unwrap_or_default())
            .unwrap_or_default();

        let sync: SyncConfig = raw.get("sync")
            .map(|s| serde_json::from_value(s.clone()).unwrap_or_default())
            .unwrap_or(SyncConfig { mode: SyncMode::Symlink });

        let install: InstallConfig = raw.get("install")
            .map(|i| serde_json::from_value(i.clone()).unwrap_or_default())
            .unwrap_or(InstallConfig {
                allow_zip: true,
                allow_git: true,
                default_sync_targets: vec![],
            });

        let exclude_paths: Vec<String> = raw.get("excludePaths")
            .map(|e| serde_json::from_value(e.clone()).unwrap_or_default())
            .unwrap_or_else(|| vec![
                "node_modules".into(), ".git".into(), "dist".into(), "coverage".into()
            ]);

        let rules: RulesConfig = raw.get("rules")
            .map(|r| serde_json::from_value(r.clone()).unwrap_or_default())
            .unwrap_or_default();

        let deleted_skills: Vec<String> = raw.get("deletedSkills")
            .map(|d| serde_json::from_value(d.clone()).unwrap_or_default())
            .unwrap_or_default();

        Ok(AppConfig {
            library_path,
            tools,
            sources,
            sync,
            install,
            exclude_paths,
            rules,
            deleted_skills,
        })
    }

    fn parse_tools(tools_json: &serde_json::Value) -> Vec<Tool> {
        let mut result = Vec::new();
        if let Some(obj) = tools_json.as_object() {
            for (name, value) in obj {
                if let Some(tool_obj) = value.as_object() {
                    let path = tool_obj.get("path")
                        .and_then(|p| p.as_str())
                        .map(|p| fs_utils::expand_tilde(p).to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let enabled = tool_obj.get("enabled")
                        .and_then(|e| e.as_bool())
                        .unwrap_or(true);
                    result.push(Tool {
                        id: name.clone(),
                        name: name.clone(),
                        path,
                        enabled,
                        is_custom: false,
                    });
                }
            }
        }
        result
    }

    pub fn save(&self) -> Result<(), AppError> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = self.to_json_value();
        let content = serde_json::to_string_pretty(&json)
            .map_err(|e| AppError::Config(format!("Failed to serialize config: {}", e)))?;
        fs::write(&path, content)?;
        Ok(())
    }

    fn to_json_value(&self) -> serde_json::Value {
        let mut tools_map = serde_json::Map::new();
        for tool in &self.tools {
            tools_map.insert(tool.id.clone(), serde_json::json!({
                "path": fs_utils::contract_tilde(&tool.path),
                "enabled": tool.enabled,
            }));
        }

        let sources: Vec<serde_json::Value> = self.sources.iter().map(|s| {
            serde_json::json!({
                "path": fs_utils::contract_tilde(&s.path),
                "group": s.group,
                "default": s.default,
                "enabled": s.enabled,
                "recursive": s.recursive,
            })
        }).collect();

        serde_json::json!({
            "library": { "path": fs_utils::contract_tilde(self.library_path.to_string_lossy().as_ref()) },
            "tools": serde_json::Value::Object(tools_map),
            "sources": sources,
            "sync": { "mode": "symlink" },
            "install": {
                "allowZip": self.install.allow_zip,
                "allowGit": self.install.allow_git,
                "defaultSyncTargets": self.install.default_sync_targets,
            },
            "excludePaths": self.exclude_paths,
            "rules": self.rules,
            "deletedSkills": self.deleted_skills,
        })
    }

    pub fn expand_tilde(path: &str) -> PathBuf {
        fs_utils::expand_tilde(path)
    }

    pub fn default_config() -> Self {
        let home = home_dir().unwrap_or_else(|| PathBuf::from("."));
        let home_str = home.to_string_lossy();

        let default_tools = vec![
            Tool { id: "cursor".into(), name: "Cursor".into(), path: format!("{}/.cursor/skills", home_str), enabled: true, is_custom: false },
            Tool { id: "claude-code".into(), name: "Claude Code".into(), path: format!("{}/.claude/skills", home_str), enabled: true, is_custom: false },
            Tool { id: "codex".into(), name: "Codex".into(), path: format!("{}/.codex/skills", home_str), enabled: true, is_custom: false },
            Tool { id: "opencode".into(), name: "OpenCode".into(), path: format!("{}/.config/opencode/skill", home_str), enabled: true, is_custom: false },
            Tool { id: "antigravity".into(), name: "Antigravity".into(), path: format!("{}/.antigravity/skills", home_str), enabled: true, is_custom: false },
            Tool { id: "gemini-cli".into(), name: "Gemini CLI".into(), path: format!("{}/.gemini/skills", home_str), enabled: true, is_custom: false },
            Tool { id: "trae-ide".into(), name: "TRAE IDE".into(), path: format!("{}/.trae/skills", home_str), enabled: true, is_custom: false },
        ];

        let default_sources = vec![
            SourceConfig {
                path: format!("{}/.agents/skills", home_str),
                group: "agents".into(),
                default: true,
                enabled: true,
                recursive: true,
            },
            SourceConfig {
                path: format!("{}/.codex/skills", home_str),
                group: "codex".into(),
                default: true,
                enabled: true,
                recursive: true,
            },
            SourceConfig {
                path: format!("{}/.claude/skills", home_str),
                group: "claude".into(),
                default: true,
                enabled: true,
                recursive: true,
            },
        ];

        AppConfig {
            library_path: Self::default_library_path(),
            tools: default_tools,
            sources: default_sources,
            sync: SyncConfig { mode: SyncMode::Symlink },
            install: InstallConfig {
                allow_zip: true,
                allow_git: true,
                default_sync_targets: vec![],
            },
            exclude_paths: vec!["node_modules".into(), ".git".into(), "dist".into(), "coverage".into()],
            rules: RulesConfig::default(),
            deleted_skills: vec![],
        }
    }

    pub fn reload(&mut self) -> Result<(), AppError> {
        let path = Self::config_path()?;
        *self = Self::load(&path)?;
        Ok(())
    }

    pub fn check_tool_availability(&mut self) -> Vec<String> {
        let mut disabled_tools = Vec::new();
        for tool in &mut self.tools {
            if !tool.enabled {
                continue;
            }
            let expanded = fs_utils::expand_tilde(&tool.path);
            if !expanded.exists() {
                tool.enabled = false;
                disabled_tools.push(tool.name.clone());
            }
        }
        if !disabled_tools.is_empty() {
            let _ = self.save();
        }
        disabled_tools
    }

    pub fn add_tool(&mut self, name: String, path: String) -> Result<(), AppError> {
        let expanded_path = fs_utils::expand_tilde(&path).to_string_lossy().into_owned();
        let id = name.to_lowercase().replace(' ', "-");
        if self.tools.iter().any(|t| t.id == id) {
            return Err(AppError::Config(format!("Tool '{}' already exists", name)));
        }
        self.tools.push(Tool {
            id,
            name,
            path: expanded_path,
            enabled: true,
            is_custom: true,
        });
        self.save()
    }

    pub fn update_tool(&mut self, tool_id: String, enabled: Option<bool>, path: Option<String>) -> Result<(), AppError> {
        let tool = self.tools.iter_mut().find(|t| t.id == tool_id)
            .ok_or_else(|| AppError::ToolNotFound(tool_id.clone()))?;
        if let Some(e) = enabled { tool.enabled = e; }
        if let Some(p) = path { tool.path = fs_utils::expand_tilde(&p).to_string_lossy().into_owned(); }
        self.save()
    }

    pub fn add_source(&mut self, source: SourceConfig) -> Result<(), AppError> {
        self.sources.push(source);
        self.save()
    }

    pub fn remove_source(&mut self, path: &str) -> Result<(), AppError> {
        self.sources.retain(|s| s.path != path);
        self.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_tilde_with_home() {
        let expanded = AppConfig::expand_tilde("~/test/path");
        assert!(!expanded.to_string_lossy().starts_with('~'));
    }

    #[test]
    fn test_expand_tilde_without_tilde() {
        let expanded = AppConfig::expand_tilde("/absolute/path");
        assert_eq!(expanded, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn test_expand_tilde_just_tilde() {
        let expanded = AppConfig::expand_tilde("~");
        assert!(!expanded.to_string_lossy().starts_with('~'));
    }

    #[test]
    fn test_parse_tools_empty() {
        let tools = AppConfig::parse_tools(&serde_json::json!({}));
        assert!(tools.is_empty());
    }

    #[test]
    fn test_parse_tools_valid() {
        let json = serde_json::json!({
            "cursor": { "path": "~/.cursor/skills", "enabled": true },
            "codex": { "path": "~/.codex/skills", "enabled": false },
        });
        let tools = AppConfig::parse_tools(&json);
        assert_eq!(tools.len(), 2);
        assert!(tools.iter().any(|tool| tool.id == "cursor" && tool.enabled));
        assert!(tools.iter().any(|tool| tool.id == "codex" && !tool.enabled));
    }

    #[test]
    fn test_default_config_values() {
        let config = AppConfig::default_config();
        assert!(!config.library_path.to_string_lossy().is_empty());
        assert!(!config.tools.is_empty());
        assert!(!config.sources.is_empty());
        assert!(config.install.allow_zip);
        assert!(config.install.allow_git);
        assert!(!config.exclude_paths.is_empty());
    }

    #[test]
    fn test_add_tool() {
        let mut config = AppConfig::default_config();
        let initial_len = config.tools.len();
        config.add_tool("Custom Tool".to_string(), "~/.custom/skills".to_string()).unwrap();
        assert_eq!(config.tools.len(), initial_len + 1);
        assert_eq!(config.tools.last().unwrap().id, "custom-tool");
    }

    #[test]
    fn test_add_tool_duplicate() {
        let mut config = AppConfig::default_config();
        let result = config.add_tool("Cursor".to_string(), "/some/path".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_update_tool() {
        let mut config = AppConfig::default_config();
        config.update_tool("cursor".to_string(), Some(false), Some("/new/path".to_string())).unwrap();
        let tool = config.tools.iter().find(|t| t.id == "cursor").unwrap();
        assert!(!tool.enabled);
        assert_eq!(tool.path, "/new/path");
    }

    #[test]
    fn test_update_tool_not_found() {
        let mut config = AppConfig::default_config();
        let result = config.update_tool("nonexistent".to_string(), Some(false), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_add_and_remove_source() {
        let mut config = AppConfig::default_config();
        let source = SourceConfig {
            path: "/new/source".to_string(),
            group: "test".to_string(),
            default: false,
            enabled: true,
            recursive: true,
        };
        config.add_source(source).unwrap();
        assert!(config.sources.iter().any(|s| s.path == "/new/source"));

        config.remove_source("/new/source").unwrap();
        assert!(!config.sources.iter().any(|s| s.path == "/new/source"));
    }

    #[test]
    fn test_from_json_value() {
        let raw = serde_json::json!({
            "library": { "path": "~/custom-library" },
            "tools": {},
            "sources": [
                { "path": "~/.agents/skills", "group": "agents", "default": true, "enabled": true, "recursive": true }
            ],
            "sync": { "mode": "symlink" },
            "install": { "allow_zip": false, "allow_git": false, "default_sync_targets": [] },
            "excludePaths": ["target"],
            "rules": { "tools": {}, "groups": {}, "skills": {} },
            "deletedSkills": [],
        });

        let config = AppConfig::from_json_value(raw).unwrap();
        assert!(!config.install.allow_zip);
        assert!(!config.install.allow_git);
        assert_eq!(config.exclude_paths, vec!["target"]);
        assert_eq!(config.sources.len(), 1);
        assert_eq!(config.sources[0].group, "agents");
    }
}
