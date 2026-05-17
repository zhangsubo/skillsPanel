import { invokeCommand } from './index';
import type { InstallCandidate } from '@/types';

export async function previewLocalInstall(
  path: string,
): Promise<InstallCandidate[]> {
  return invokeCommand<InstallCandidate[]>('preview_local_install', { path });
}

export async function previewGitInstall(
  gitUrl: string,
  subpath?: string,
): Promise<InstallCandidate[]> {
  return invokeCommand<InstallCandidate[]>('preview_git_install', {
    gitUrl,
    ...(subpath !== undefined && { subpath }),
  });
}

export async function installLocalSkill(
  sourcePath: string,
  name?: string,
): Promise<string> {
  return invokeCommand<string>('install_local_skill', {
    sourcePath,
    ...(name !== undefined && { name }),
  });
}

export async function installGitSkill(
  gitUrl: string,
  subpath?: string,
  name?: string,
): Promise<string> {
  return invokeCommand<string>('install_git_skill', {
    gitUrl,
    ...(subpath !== undefined && { subpath }),
    ...(name !== undefined && { name }),
  });
}

export async function checkSkillUpdate(skillId: string): Promise<boolean> {
  return invokeCommand<boolean>('check_skill_update', { skillId });
}

export async function updateSkill(skillId: string): Promise<string> {
  return invokeCommand<string>('update_skill', { skillId });
}

