import { describe, test, expect, vi, beforeEach } from 'vitest'
import { render, waitFor, act } from '@testing-library/react'
import { useEffect } from 'react'

// Mock the Tauri-side API before the hook imports it.
vi.mock('@/api/tags', () => ({
  listTags: vi.fn(),
  createTag: vi.fn(),
  updateTag: vi.fn(),
  deleteTag: vi.fn(),
  attachTag: vi.fn(),
  detachTag: vi.fn(),
  bulkAttachTag: vi.fn(),
  getSkillTags: vi.fn(),
  getAllSkillTags: vi.fn(),
}))

import * as tagsApi from '@/api/tags'
import { useTags } from '@/hooks/use-tags'
import type { Tag } from '@/types'

// ── Fixtures ─────────────────────────────────────────────────────

function makeTag(id: string, name: string, color: string | null = null): Tag {
  return {
    id,
    name,
    color,
    description: null,
    created_at: '2024-01-01T00:00:00Z',
  }
}

/** Render the hook and capture the latest return value via a probe component. */
function Probe({ onChange }: { onChange: (state: ReturnType<typeof useTags>) => void }) {
  const state = useTags()
  useEffect(() => {
    onChange(state)
  })
  return null
}

let lastState: ReturnType<typeof useTags> | null = null

function renderHook() {
  return render(<Probe onChange={(s) => (lastState = s)} />)
}

// ── Tests ────────────────────────────────────────────────────────

describe('useTags', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    lastState = null
  })

  test('initial mount: fetches tags via listTags', async () => {
    // Arrange
    const tags = [makeTag('t1', 'rust', '#dea584'), makeTag('t2', 'frontend')]
    vi.mocked(tagsApi.listTags).mockResolvedValue(tags)

    // Act
    renderHook()

    // Assert
    await waitFor(() => {
      expect(lastState).not.toBeNull()
    })
    await waitFor(() => {
      expect(lastState?.tags).toEqual(tags)
    })
    expect(lastState?.loading).toBe(false)
    expect(lastState?.error).toBeNull()
  })

  test('create() appends the new tag to local state and keeps sort by name', async () => {
    // Arrange
    vi.mocked(tagsApi.listTags).mockResolvedValue([makeTag('t1', 'mike')])
    const newTag = makeTag('t2', 'alpha')
    vi.mocked(tagsApi.createTag).mockResolvedValue(newTag)

    renderHook()
    await waitFor(() => expect(lastState?.tags.length).toBe(1))

    // Act
    await act(async () => {
      await lastState!.create('alpha')
    })

    // Assert: local state has both, sorted by name
    expect(lastState?.tags.map((t) => t.name)).toEqual(['alpha', 'mike'])
  })

  test('update() passes undefined for fields not given, triggers refresh', async () => {
    // Arrange
    const initial = [makeTag('t1', 'old', '#000000')]
    vi.mocked(tagsApi.listTags).mockResolvedValue(initial)
    const refreshed = [makeTag('t1', 'new', '#ffffff')]
    vi.mocked(tagsApi.listTags)
      .mockResolvedValueOnce(initial)
      .mockResolvedValueOnce(refreshed)
    vi.mocked(tagsApi.updateTag).mockResolvedValue(undefined)

    renderHook()
    await waitFor(() => expect(lastState?.tags.length).toBe(1))

    // Act: change only name; color/description must be passed as undefined
    await act(async () => {
      await lastState!.update('t1', { name: 'new' })
    })

    // Assert: updateTag was called with only the changed field
    expect(tagsApi.updateTag).toHaveBeenCalledWith('t1', 'new', undefined, undefined)
    // And refresh() pulled the updated list
    await waitFor(() => {
      expect(lastState?.tags[0].name).toBe('new')
      expect(lastState?.tags[0].color).toBe('#ffffff')
    })
  })

  test('remove() optimistically drops the tag from local state', async () => {
    // Arrange
    const tags = [makeTag('t1', 'rust'), makeTag('t2', 'frontend')]
    vi.mocked(tagsApi.listTags).mockResolvedValue(tags)
    vi.mocked(tagsApi.deleteTag).mockResolvedValue(undefined)

    renderHook()
    await waitFor(() => expect(lastState?.tags.length).toBe(2))

    // Act
    await act(async () => {
      await lastState!.remove('t1')
    })

    // Assert
    expect(lastState?.tags).toEqual([tags[1]])
  })

  test('fetchAllSkillTagsMap() groups rows by skill_id', async () => {
    // Arrange
    vi.mocked(tagsApi.listTags).mockResolvedValue([])
    const t_a = makeTag('t-a', 'a')
    const t_b = makeTag('t-b', 'b')
    vi.mocked(tagsApi.getAllSkillTags).mockResolvedValue([
      ['s1', t_a],
      ['s1', t_b],
      ['s2', t_a],
    ])

    renderHook()
    await waitFor(() => expect(lastState).not.toBeNull())

    // Act
    const map = await lastState!.fetchAllSkillTagsMap()

    // Assert
    expect(map.size).toBe(2)
    expect(map.get('s1')?.map((t) => t.name)).toEqual(['a', 'b'])
    expect(map.get('s2')?.map((t) => t.name)).toEqual(['a'])
  })
})
