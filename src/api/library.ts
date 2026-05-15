import { invokeCommand } from './index';
import type { ScanResult, SkillWithStatus, Tool } from '@/types';

export async function getLibrary(): Promise<string[]> {
  return invokeCommand<string[]>('get_library');
}

export async function scanSkills(): Promise<ScanResult> {
  return invokeCommand<ScanResult>('scan_skills');
}

export async function getSkillContent(skillId: string): Promise<string> {
  return invokeCommand<string>('get_skill_content', { skillId });
}

export async function getTools(): Promise<Tool[]> {
  return invokeCommand<Tool[]>('get_tools');
}

export async function linkSkill(skillName: string, toolId: string): Promise<void> {
  return invokeCommand<void>('link_skill', { skillName, toolId });
}

export async function unlinkSkill(skillName: string, toolId: string): Promise<void> {
  return invokeCommand<void>('unlink_skill', { skillName, toolId });
}

export async function deleteSkill(skillName: string): Promise<void> {
  return invokeCommand<void>('delete_skill', { skillName, hardDelete: true });
}

export async function batchDeleteSkills(skillNames: string[], deleteSymlinks?: boolean): Promise<number> {
  return invokeCommand<number>('batch_delete_skills', {
    skillNames,
    deleteSymlinks: deleteSymlinks ?? false,
  });
}

export interface ScanDiff {
  added: SkillWithStatus[];
  updated: SkillWithStatus[];
  deleted: SkillWithStatus[];
}

export async function getScanDiff(): Promise<ScanDiff> {
  return invokeCommand<ScanDiff>('get_scan_diff');
}