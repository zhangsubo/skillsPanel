import { invokeCommand } from './index';

export async function exportSkill(
  skillName: string,
  targetPath: string,
  asZip: boolean,
): Promise<void> {
  return invokeCommand<void>('export_skill', { skillName, targetPath, asZip });
}

export async function batchExportSkills(
  skillNames: string[],
  targetPath: string,
  asZip: boolean,
): Promise<number> {
  return invokeCommand<number>('batch_export_skills', {
    skillNames,
    targetPath,
    asZip,
  });
}