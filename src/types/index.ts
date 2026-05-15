// TypeScript type definitions matching Rust models in src-tauri/src/core/models.rs
// All enum variants use kebab-case to match serde(rename_all = "kebab-case")

// ── Union types (Rust enums with serde kebab-case) ──

export type SkillSourceType = 'local-folder' | 'local-zip' | 'git';

export type SkillToolStatus = 'linked' | 'missing' | 'wrong' | 'directory' | 'blocked' | 'error';

export type RuleSource = 'tool' | 'group' | 'skill' | 'default';

export type SyncMode = 'symlink' | 'copy';

export type DryRunActionType = 'create-link' | 'remove-link' | 'fix-link' | 'skip-blocked' | 'skip-directory';

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