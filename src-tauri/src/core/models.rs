use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub path_hash: String,
    pub library_path: String,
    pub original_source_path: Option<String>,
    pub original_git_url: Option<String>,
    pub original_git_subpath: Option<String>,
    pub group: String,
    pub description: String,
    pub frontmatter: HashMap<String, serde_json::Value>,
    pub created_at: String,
    pub mtime_ms: i64,
    pub source_type: SkillSourceType,
    pub is_deleted: bool,
    pub content_hash: Option<String>,
    #[serde(default)]
    pub source_revision: Option<String>,
    #[serde(default)]
    pub source_remote_revision: Option<String>,
    #[serde(default)]
    pub source_update_status: SourceUpdateStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SkillSourceType {
    #[default]
    LocalFolder,
    LocalZip,
    Git,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceUpdateStatus {
    UpToDate,
    UpdateAvailable,
    Unknown,
}

impl Default for SourceUpdateStatus {
    fn default() -> Self {
        Self::UpToDate
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub id: String,
    pub name: String,
    pub path: String,
    pub enabled: bool,
    pub is_custom: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallCandidate {
    pub candidate_id: String,
    pub detected_name: Option<String>,
    pub user_name_override: Option<String>,
    pub description: Option<String>,
    pub source_path: String,
    pub skill_root: String,
    pub valid: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SkillToolStatus {
    Linked,
    Missing,
    Wrong,
    Directory,
    Blocked,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleDecision {
    pub allowed: bool,
    pub reason: Option<String>,
    pub source: Option<RuleSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum RuleSource {
    Tool,
    Group,
    Skill,
    Default,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillWithStatus {
    pub skill: Skill,
    pub tool_statuses: HashMap<String, SkillToolStatus>,
    pub rule_decisions: HashMap<String, RuleDecision>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SyncMode {
    #[default]
    Symlink,
    Copy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: String,
    pub action: String,
    pub target: String,
    pub details: Option<String>,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolRule {
    #[serde(default)]
    pub block_all: bool,
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub allow_groups: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GroupRule {
    #[serde(default)]
    pub only: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillRule {
    #[serde(default)]
    pub only: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceConfig {
    pub path: String,
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub default: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub recursive: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncConfig {
    #[serde(default = "default_symlink")]
    pub mode: SyncMode,
}

fn default_symlink() -> SyncMode {
    SyncMode::Symlink
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InstallConfig {
    #[serde(default = "default_true")]
    pub allow_zip: bool,
    #[serde(default = "default_true")]
    pub allow_git: bool,
    #[serde(default)]
    pub default_sync_targets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RulesConfig {
    #[serde(default)]
    pub tools: HashMap<String, ToolRule>,
    #[serde(default)]
    pub groups: HashMap<String, GroupRule>,
    #[serde(default)]
    pub skills: HashMap<String, SkillRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub skills: Vec<SkillWithStatus>,
    pub total_skills: usize,
    pub total_tools: usize,
    pub linked_count: usize,
    pub conflict_count: usize,
    pub blocked_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DryRunResult {
    pub actions: Vec<DryRunAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DryRunActionType {
    CreateLink,
    RemoveLink,
    FixLink,
    SkipBlocked,
    SkipDirectory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DryRunAction {
    pub action_type: DryRunActionType,
    pub skill_name: String,
    pub tool_name: String,
    pub source_path: String,
    pub target_path: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
    pub source: String,
}

// ── Project / Workspace Models ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub root_path: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncHealthStatus {
    InSync,
    CenterNewer,
    ProjectNewer,
    Diverged,
    ProjectOnly,
    CenterOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSkillInfo {
    pub name: String,
    pub description: String,
    pub relative_path: String,
    pub agent: String,
    pub enabled: bool,
    pub content_hash: Option<String>,
    pub in_center: bool,
    pub center_skill_id: Option<String>,
    pub sync_status: SyncHealthStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncHealthDto {
    pub in_sync: usize,
    pub center_newer: usize,
    pub project_newer: usize,
    pub diverged: usize,
    pub project_only: usize,
    pub center_only: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDto {
    pub project: Project,
    pub skills: Vec<ProjectSkillInfo>,
    pub sync_health: SyncHealthDto,
}
