import { invokeCommand } from './index';
import type { Project, ProjectDto } from '@/types';

export async function listProjects(): Promise<Project[]> {
  return invokeCommand<Project[]>('list_projects');
}

export async function createProject(name: string, rootPath: string): Promise<void> {
  return invokeCommand<void>('create_project', { name, rootPath });
}

export async function deleteProject(projectId: string): Promise<void> {
  return invokeCommand<void>('delete_project', { projectId });
}

export async function scanProject(projectId: string): Promise<ProjectDto> {
  return invokeCommand<ProjectDto>('scan_project', { projectId });
}

export async function importProjectSkill(projectId: string, skillName: string): Promise<string> {
  return invokeCommand<string>('import_project_skill', { projectId, skillName });
}

export async function exportSkillToProject(projectId: string, skillName: string, agent: string): Promise<void> {
  return invokeCommand<void>('export_skill_to_project', { projectId, skillName, agent });
}
