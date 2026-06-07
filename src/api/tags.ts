import { invokeCommand } from './index';
import type { Tag } from '@/types';

/**
 * 标签相关 Tauri 命令的薄壳。所有函数都走 invokeCommand，
 * 在浏览器开发模式下会落到 MOCK_COMMANDS（见 src/api/index.ts）。
 *
 * 约定：
 * - `update` 形参里 `null` 表示「不动该列」，`""` 表示「清空该列」。
 *   与 Rust 端 `TagsRepository::update` 的语义保持一致。
 * - 失败统一抛 Error（invokeCommand 已包）。
 */
export async function listTags(): Promise<Tag[]> {
  return invokeCommand<Tag[]>('list_tags');
}

export async function createTag(
  name: string,
  color?: string | null,
  description?: string | null,
): Promise<Tag> {
  return invokeCommand<Tag>('create_tag', { name, color, description });
}

export async function updateTag(
  id: string,
  name?: string | null,
  color?: string | null,
  description?: string | null,
): Promise<void> {
  return invokeCommand<void>('update_tag', { id, name, color, description });
}

export async function deleteTag(id: string): Promise<void> {
  return invokeCommand<void>('delete_tag', { id });
}

export async function attachTag(skillId: string, tagId: string): Promise<void> {
  return invokeCommand<void>('attach_tag', { skillId, tagId });
}

export async function detachTag(skillId: string, tagId: string): Promise<void> {
  return invokeCommand<void>('detach_tag', { skillId, tagId });
}

export async function bulkAttachTag(skillIds: string[], tagId: string): Promise<void> {
  return invokeCommand<void>('bulk_attach_tag', { skillIds, tagId });
}

export async function getSkillTags(skillId: string): Promise<Tag[]> {
  return invokeCommand<Tag[]>('get_skill_tags', { skillId });
}

/**
 * Bulk variant for the Library page. Returns one entry per (skill_id, tag)
 * link — callers group into `Map<skillId, Tag[]>` for fast chip rendering.
 * Skills with no tags are absent from the array.
 */
export async function getAllSkillTags(): Promise<Array<[string, Tag]>> {
  return invokeCommand<Array<[string, Tag]>>('get_all_skill_tags');
}
