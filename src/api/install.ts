import { invokeCommand } from './index';
import type { InstallCandidate } from '@/types';

export async function previewLocalInstall(
  path: string,
): Promise<InstallCandidate[]> {
  return invokeCommand<InstallCandidate[]>('preview_local_install', { path });
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

