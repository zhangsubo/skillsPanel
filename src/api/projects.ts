import { invokeCommand } from './index';
import type { Project, ProjectDto } from '@/types';

export async function listProjects(): Promise<Project[]> {
  return invokeCommand<Project[]>('list_projects');
}

export async function createProject(name: string, rootPath: string): Promise<string> {
  return invokeCommand<string>('create_project', { name, rootPath });
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

/**
 * 从项目中删除 skill
 */
export async function deleteProjectSkill(
  projectId: string,
  skillName: string,
  agent: string
): Promise<void> {
  return invokeCommand<void>('delete_project_skill', { projectId, skillName, agent });
}

/**
 * 批量导出 skill 到多个 agent
 */
export async function exportSkillToProjectMulti(
  projectId: string,
  skillName: string,
  agents: string[]
): Promise<string[]> {
  return invokeCommand<string[]>('export_skill_to_project_multi', {
    projectId,
    skillName,
    agents,
  });
}

/**
 * 更新项目 skill 的 agent 目标
 */
export async function updateProjectSkillAgents(
  projectId: string,
  skillName: string,
  currentAgents: string[],
  targetAgents: string[]
): Promise<{
  added: string[];
  removed: string[];
  unchanged: string[];
}> {
  return invokeCommand('update_project_skill_agents', {
    projectId,
    skillName,
    currentAgents,
    targetAgents,
  });
}
