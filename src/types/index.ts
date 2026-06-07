// TypeScript type definitions matching Rust models in src-tauri/src/core/models.rs
// All enum variants use kebab-case to match serde(rename_all = "kebab-case")

// ── Union types (Rust enums with serde kebab-case) ──

export type SkillSourceType = 'local-folder' | 'local-zip' | 'git';

export type SkillToolStatus = 'linked' | 'missing' | 'wrong' | 'directory' | 'blocked' | 'error';

export type RuleSource = 'tool' | 'group' | 'skill' | 'default';

export type SyncMode = 'symlink' | 'copy';

export type DryRunActionType = 'create-link' | 'remove-link' | 'fix-link' | 'skip-blocked' | 'skip-directory';

export type SourceUpdateStatus = 'up-to-date' | 'update-available' | 'unknown';

// ── Interfaces (Rust structs) ──

export interface Skill {
  id: string;
  name: string;
  path_hash: string;
  library_path: string;
  original_source_path: string | null;
  original_git_url: string | null;
  original_git_subpath: string | null;
  group: string;
  description: string;
  frontmatter: Record<string, unknown>;
  created_at: string;
  mtime_ms: number;
  source_type: SkillSourceType;
  is_deleted: boolean;
  source_revision: string | null;
  source_remote_revision: string | null;
  source_update_status: SourceUpdateStatus;
}

// ── Tags (user-defined skill grouping) ────────────────────────────
// Tags are stored only in the local DB. They never modify SKILL.md.

export interface Tag {
  id: string;
  name: string;
  color: string | null;
  description: string | null;
  created_at: string;
}

export interface Tool {
  id: string;
  name: string;
  path: string;
  enabled: boolean;
  is_custom: boolean;
}

export interface InstallCandidate {
  candidate_id: string;
  detected_name: string | null;
  user_name_override: string | null;
  description: string | null;
  source_path: string;
  skill_root: string;
  valid: boolean;
  error: string | null;
}

export interface RuleDecision {
  allowed: boolean;
  reason: string | null;
  source: RuleSource | null;
}

export interface Tag {
  id: string;
  name: string;
  color: string | null;
  description: string | null;
  created_at: string;
}

export interface SkillWithStatus {
  skill: Skill;
  tool_statuses: Record<string, SkillToolStatus>;
  rule_decisions: Record<string, RuleDecision>;
}

export interface AuditEntry {
  timestamp: string;
  action: string;
  target: string;
  details: string | null;
  success: boolean;
  error: string | null;
}

export interface LogEntry {
  timestamp: string;
  level: string;
  message: string;
  source: string;
}

export interface ToolRule {
  block_all: boolean;
  allow: string[];
  allow_groups: string[];
}

export interface GroupRule {
  only: string[];
  exclude: string[];
}

export interface SkillRule {
  only: string[];
  exclude: string[];
}

export interface SourceConfig {
  path: string;
  group: string;
  default: boolean;
  enabled: boolean;
  recursive: boolean;
}

export interface SyncConfig {
  mode: SyncMode;
}

export interface InstallConfig {
  allow_zip: boolean;
  allow_git: boolean;
  default_sync_targets: string[];
}

export interface RulesConfig {
  tools: Record<string, ToolRule>;
  groups: Record<string, GroupRule>;
  skills: Record<string, SkillRule>;
}

export interface ScanResult {
  skills: SkillWithStatus[];
  total_skills: number;
  total_tools: number;
  linked_count: number;
  conflict_count: number;
  blocked_count: number;
}

export interface DryRunResult {
  actions: DryRunAction[];
}

export interface DryRunAction {
  action_type: DryRunActionType;
  skill_name: string;
  tool_name: string;
  source_path: string;
  target_path: string;
  reason: string | null;
}

export interface AppConfig {
  sources: SourceConfig[];
  sync: SyncConfig;
  install: InstallConfig;
  rules: RulesConfig;
}

export interface InstallProgress {
  stage: string;
  message: string;
}

// ── Project / Workspace Types ─────────────────────────────────────

export type SyncHealthStatus = 'in_sync' | 'center_newer' | 'project_newer' | 'diverged' | 'project_only' | 'center_only';

export interface Project {
  id: string;
  name: string;
  root_path: string;
  created_at: string;
  updated_at: string;
}

export interface ProjectSkillInfo {
  name: string;
  description: string;
  relative_path: string;
  agent: string;
  enabled: boolean;
  content_hash: string | null;
  in_center: boolean;
  center_skill_id: string | null;
  sync_status: SyncHealthStatus;
}

export interface SyncHealthDto {
  in_sync: number;
  center_newer: number;
  project_newer: number;
  diverged: number;
  project_only: number;
  center_only: number;
}

export interface ProjectDto {
  project: Project;
  skills: ProjectSkillInfo[];
  sync_health: SyncHealthDto;
}

// ── Cloud sync (user-defined providers) ────────────────────────────
// Mirrors `crate::core::models::{SyncProvider, SyncHistory}`. Field
// names follow snake_case (Rust serde) since the backend is the source
// of truth. Keep these in sync with `src-tauri/src/core/models.rs`.

export type SyncProviderKind = 'github_zip' | 'webdav';

export interface SyncProvider {
  id: string;
  name: string;
  kind: string;
  config_json: string;
  enabled: boolean;
  last_sync_at: string | null;
  last_sync_status: string | null;
  last_sync_error: string | null;
  created_at: string;
}

export type SyncDirection = 'upload' | 'download';

export type SyncStatus = 'success' | 'error' | 'cancelled' | 'in_progress';

export interface SyncHistory {
  id: string;
  provider_id: string;
  direction: string;
  status: string;
  started_at: string;
  finished_at: string | null;
  bytes_transferred: number | null;
  skills_count: number | null;
  error_message: string | null;
}