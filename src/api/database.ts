import { invokeCommand } from './index';
import type { Skill, Tool, AuditEntry, LogEntry } from '@/types';

export async function getInstalledSkillsFromDb(): Promise<Skill[]> {
  return invokeCommand<Skill[]>('get_installed_skills_from_db');
}

export async function getAllActiveSkillsFromDb(): Promise<Skill[]> {
  return invokeCommand<Skill[]>('get_all_active_skills_from_db');
}

export async function markSkillInstalled(skillId: string): Promise<void> {
  return invokeCommand<void>('mark_skill_installed', { skillId });
}

export async function markSkillUninstalled(skillName: string): Promise<void> {
  return invokeCommand<void>('mark_skill_uninstalled', { skillName });
}

export async function upsertSkillToDb(skill: Skill): Promise<void> {
  return invokeCommand<void>('upsert_skill_to_db', { skill });
}

export async function getConfigValue(key: string): Promise<string | null> {
  return invokeCommand<string | null>('get_config_value', { key });
}

export async function setConfigValue(key: string, value: string): Promise<void> {
  return invokeCommand<void>('set_config_value', { key, value });
}

export async function getAuditLogsFromDb(limit?: number): Promise<AuditEntry[]> {
  return invokeCommand<AuditEntry[]>('get_audit_logs_from_db', { limit });
}

export async function logAuditEntry(
  action: string,
  target: string,
  details?: string,
  success: boolean = true,
  error?: string
): Promise<void> {
  return invokeCommand<void>('log_audit_entry', { action, target, details, success, error });
}

export async function getAppLogsFromDb(limit?: number): Promise<LogEntry[]> {
  return invokeCommand<LogEntry[]>('get_app_logs_from_db', { limit });
}

export async function logAppMessage(level: string, message: string, source: string): Promise<void> {
  return invokeCommand<void>('log_app_message', { level, message, source });
}

export async function getToolsFromDb(): Promise<Tool[]> {
  return invokeCommand<Tool[]>('get_tools_from_db');
}

export async function upsertToolToDb(tool: Tool): Promise<void> {
  return invokeCommand<void>('upsert_tool_to_db', { tool });
}

export async function linkToolSkillInDb(toolId: string, skillId: string): Promise<void> {
  return invokeCommand<void>('link_tool_skill_in_db', { toolId, skillId });
}

export async function unlinkToolSkillInDb(toolId: string, skillId: string): Promise<void> {
  return invokeCommand<void>('unlink_tool_skill_in_db', { toolId, skillId });
}

export async function getLinkedToolIds(skillId: string): Promise<string[]> {
  return invokeCommand<string[]>('get_linked_tool_ids', { skillId });
}
