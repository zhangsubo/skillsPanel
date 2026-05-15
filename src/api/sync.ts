import { invokeCommand } from './index';

export async function syncSkills(skillNames?: string[]): Promise<number> {
  return invokeCommand<number>('sync_skills', {
    ...(skillNames !== undefined && { skillNames }),
  });
}

export async function cleanStaleLinks(): Promise<number> {
  return invokeCommand<number>('clean_stale_links');
}