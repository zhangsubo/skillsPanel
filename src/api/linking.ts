import { invokeCommand } from './index';

export async function linkSkill(
  skillName: string,
  toolId: string,
): Promise<void> {
  return invokeCommand<void>('link_skill', { skillName, toolId });
}

export async function unlinkSkill(
  skillName: string,
  toolId: string,
): Promise<void> {
  return invokeCommand<void>('unlink_skill', { skillName, toolId });
}

export async function fixSkillLink(
  skillName: string,
  toolId: string,
): Promise<void> {
  return invokeCommand<void>('fix_skill_link', { skillName, toolId });
}

export async function batchLinkSkills(
  skillNames: string[],
  toolId: string,
): Promise<number> {
  return invokeCommand<number>('batch_link_skills', { skillNames, toolId });
}

export async function batchUnlinkSkills(
  skillNames: string[],
  toolId: string,
): Promise<number> {
  return invokeCommand<number>('batch_unlink_skills', { skillNames, toolId });
}