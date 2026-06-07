import { useCallback, useEffect, useState } from 'react';
import {
  attachTag,
  bulkAttachTag,
  createTag,
  deleteTag,
  detachTag,
  getAllSkillTags,
  getSkillTags,
  listTags,
  updateTag,
} from '@/api/tags';
import type { Tag } from '@/types';

interface UseTagsState {
  tags: Tag[];
  loading: boolean;
  error: Error | null;
  refresh: () => Promise<void>;
  create: (name: string, color?: string | null, description?: string | null) => Promise<Tag>;
  update: (id: string, fields: { name?: string; color?: string | null; description?: string | null }) => Promise<void>;
  remove: (id: string) => Promise<void>;
  attach: (skillId: string, tagId: string) => Promise<void>;
  detach: (skillId: string, tagId: string) => Promise<void>;
  bulkAttach: (skillIds: string[], tagId: string) => Promise<void>;
  tagsForSkill: (skillId: string) => Promise<Tag[]>;
  /** Bulk fetch for the Library page. Returns `Map<skillId, Tag[]>` (skills with no tags are omitted). */
  fetchAllSkillTagsMap: () => Promise<Map<string, Tag[]>>;
}

/**
 * 集中管理标签 CRUD + 关联操作。组件订阅 tags 列表；
 * 单个 skill 的标签按需通过 tagsForSkill() 拉取，避免 N+1。
 */
export function useTags(): UseTagsState {
  const [tags, setTags] = useState<Tag[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<Error | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await listTags();
      setTags(list);
    } catch (e) {
      setError(e as Error);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const create = useCallback<UseTagsState['create']>(
    async (name, color, description) => {
      const tag = await createTag(name, color, description);
      setTags((prev) => [...prev, tag].sort((a, b) => a.name.localeCompare(b.name)));
      return tag;
    },
    [],
  );

  const update = useCallback<UseTagsState['update']>(
    async (id, fields) => {
      await updateTag(
        id,
        fields.name,
        fields.color === undefined ? undefined : fields.color,
        fields.description === undefined ? undefined : fields.description,
      );
      await refresh();
    },
    [refresh],
  );

  const remove = useCallback<UseTagsState['remove']>(
    async (id) => {
      await deleteTag(id);
      setTags((prev) => prev.filter((t) => t.id !== id));
    },
    [],
  );

  const attach = useCallback<UseTagsState['attach']>(async (skillId, tagId) => {
    await attachTag(skillId, tagId);
  }, []);

  const detach = useCallback<UseTagsState['detach']>(async (skillId, tagId) => {
    await detachTag(skillId, tagId);
  }, []);

  const bulkAttach = useCallback<UseTagsState['bulkAttach']>(async (skillIds, tagId) => {
    await bulkAttachTag(skillIds, tagId);
  }, []);

  const tagsForSkill = useCallback<UseTagsState['tagsForSkill']>(async (skillId) => {
    return getSkillTags(skillId);
  }, []);

  const fetchAllSkillTagsMap = useCallback<UseTagsState['fetchAllSkillTagsMap']>(async () => {
    const rows = await getAllSkillTags();
    const map = new Map<string, Tag[]>();
    for (const [skillId, tag] of rows) {
      const list = map.get(skillId) ?? [];
      list.push(tag);
      map.set(skillId, list);
    }
    return map;
  }, []);

  return {
    tags,
    loading,
    error,
    refresh,
    create,
    update,
    remove,
    attach,
    detach,
    bulkAttach,
    tagsForSkill,
    fetchAllSkillTagsMap,
  };
}
