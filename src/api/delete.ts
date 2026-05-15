import { invokeCommand } from './index';

export async function deleteSkill(
  skillName: string,
  hardDelete: boolean,
): Promise<void> {
  return invokeCommand<void>('delete_skill', { skillName, hardDelete });
}

export async function restoreSkill(skillName: string): Promise<void> {
  return invokeCommand<void>('restore_skill', { skillName });
}

export async function batchDeleteSkills(
  skillNames: string[],
  deleteSymlinks: boolean,
): Promise<number> {
  return invokeCommand<number>('batch_delete_skills', {
    skillNames,
    deleteSymlinks,
  });
}