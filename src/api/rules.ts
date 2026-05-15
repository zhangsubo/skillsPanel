import { invokeCommand } from './index';
import type { SkillRule, GroupRule, ToolRule } from '@/types';

export async function updateSkillRule(
  skillName: string,
  rule: SkillRule,
): Promise<void> {
  return invokeCommand<void>('update_skill_rule', { skillName, rule });
}

export async function updateGroupRule(
  group: string,
  rule: GroupRule,
): Promise<void> {
  return invokeCommand<void>('update_group_rule', { group, rule });
}

export async function updateToolRule(
  toolId: string,
  rule: ToolRule,
): Promise<void> {
  return invokeCommand<void>('update_tool_rule', { toolId, rule });
}