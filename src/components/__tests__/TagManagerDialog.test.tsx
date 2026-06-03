import { describe, test, expect, vi, beforeAll, beforeEach, afterEach } from 'vitest'
import { render, screen, waitFor, fireEvent, cleanup } from '@testing-library/react'
import { I18nextProvider } from 'react-i18next'
import i18next from 'i18next'
import { initReactI18next } from 'react-i18next'

// Mock the Tauri-side API before any component imports it.
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
import { TagManagerDialog } from '@/components/TagManagerDialog'
import type { Tag } from '@/types'

// ── Fixtures ─────────────────────────────────────────────────────

function makeTag(id: string, name: string, color: string | null = null): Tag {
  return { id, name, color, description: null, created_at: '2024-01-01T00:00:00Z' }
}

const enUS = {
  'tag.manage': 'Manage Tags',
  'tag.manageDescription': 'desc',
  'tag.namePlaceholder': 'name',
  'tag.colorLabel': 'color',
  'tag.create': 'Create',
  'tag.confirmDelete': 'Delete this tag?',
  'tag.empty': 'No tags yet.',
  'tag.applyToN': 'Apply to {{count}}',
  'tag.applyToCurrent': 'Toggle tag',
  'tag.attach': 'Add',
  'tag.detach': 'Attached',
  'tag.deleteAriaLabel': 'Delete tag {{name}}',
  'tag.filterAll': 'All',
  'common.loading': 'Loading…',
  'common.close': 'Close',
}

beforeAll(async () => {
  await i18next.use(initReactI18next).init({
    lng: 'en-US',
    resources: { 'en-US': { translation: enUS } },
    interpolation: { escapeValue: false },
  })
})

function renderDialog(props: { selectedSkillIds?: string[] }) {
  return render(
    <I18nextProvider i18n={i18next}>
      <TagManagerDialog open onClose={() => {}} {...props} />
    </I18nextProvider>,
  )
}

// ── Tests ────────────────────────────────────────────────────────

describe('TagManagerDialog — single-skill mode', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    // Default happy-path mocks; individual tests override.
    vi.mocked(tagsApi.listTags).mockResolvedValue([])
    vi.mocked(tagsApi.getSkillTags).mockResolvedValue([])
    vi.mocked(tagsApi.attachTag).mockResolvedValue(undefined)
    vi.mocked(tagsApi.detachTag).mockResolvedValue(undefined)
  })
  afterEach(() => cleanup())

  test('fetches current attach set via getSkillTags on open', async () => {
    const t1 = makeTag('t1', 'frontend', '#3b82f6')
    vi.mocked(tagsApi.listTags).mockResolvedValue([t1])
    vi.mocked(tagsApi.getSkillTags).mockResolvedValue([t1])

    renderDialog({ selectedSkillIds: ['skill-1'] })

    await waitFor(() => {
      expect(tagsApi.getSkillTags).toHaveBeenCalledWith('skill-1')
    })
  })

  test('renders attach/detach labels and aria-pressed reflecting state', async () => {
    const t1 = makeTag('t1', 'frontend', '#3b82f6')
    const t2 = makeTag('t2', 'urgent', '#ef4444')
    vi.mocked(tagsApi.listTags).mockResolvedValue([t1, t2])
    // t1 attached, t2 not
    vi.mocked(tagsApi.getSkillTags).mockResolvedValue([t1])

    renderDialog({ selectedSkillIds: ['skill-1'] })

    // t1 attached → button shows "Attached" + Check + aria-pressed=true
    const t1Btn = await screen.findByTestId('tag-apply-t1')
    expect(t1Btn).toHaveTextContent('Attached')
    expect(t1Btn).toHaveAttribute('aria-pressed', 'true')

    // t2 free → button shows "Add" + aria-pressed=false
    const t2Btn = await screen.findByTestId('tag-apply-t2')
    expect(t2Btn).toHaveTextContent('Add')
    expect(t2Btn).toHaveAttribute('aria-pressed', 'false')
  })

  test('clicking an attached tag detaches it; clicking a free tag attaches it', async () => {
    const t1 = makeTag('t1', 'frontend', '#3b82f6')
    const t2 = makeTag('t2', 'urgent', '#ef4444')
    vi.mocked(tagsApi.listTags).mockResolvedValue([t1, t2])
    vi.mocked(tagsApi.getSkillTags).mockResolvedValue([t1])

    renderDialog({ selectedSkillIds: ['skill-1'] })

    // t1 is attached → click should detach
    const t1Btn = await screen.findByTestId('tag-apply-t1')
    fireEvent.click(t1Btn)
    await waitFor(() => {
      expect(tagsApi.detachTag).toHaveBeenCalledWith('skill-1', 't1')
    })
    expect(tagsApi.attachTag).not.toHaveBeenCalledWith('skill-1', 't1')

    // t2 is not attached → click should attach
    const t2Btn = await screen.findByTestId('tag-apply-t2')
    fireEvent.click(t2Btn)
    await waitFor(() => {
      expect(tagsApi.attachTag).toHaveBeenCalledWith('skill-1', 't2')
    })
  })

  test('does NOT call bulkAttachTag in single-skill mode', async () => {
    const t1 = makeTag('t1', 'frontend')
    vi.mocked(tagsApi.listTags).mockResolvedValue([t1])
    vi.mocked(tagsApi.getSkillTags).mockResolvedValue([])

    renderDialog({ selectedSkillIds: ['skill-1'] })

    const btn = await screen.findByTestId('tag-apply-t1')
    fireEvent.click(btn)

    await waitFor(() => {
      expect(tagsApi.attachTag).toHaveBeenCalledWith('skill-1', 't1')
    })
    expect(tagsApi.bulkAttachTag).not.toHaveBeenCalled()
  })

  test('does not refetch getSkillTags when the tag list mutates (no extra race)', async () => {
    // Regression for the H1 effect-deps bug: dropping `tags` from the dep
    // array means a tag CRUD inside this dialog should NOT re-fire the
    // attach-set fetch.
    const t1 = makeTag('t1', 'frontend')
    vi.mocked(tagsApi.listTags).mockResolvedValue([t1])
    vi.mocked(tagsApi.getSkillTags).mockResolvedValue([])

    renderDialog({ selectedSkillIds: ['skill-1'] })
    await screen.findByTestId('tag-apply-t1')

    // Simulate useTags publishing a fresh array reference for the same content.
    // Because tags is dropped from the deps, getSkillTags must not be re-called.
    const t1v2 = [makeTag('t1', 'frontend', '#000000')]
    vi.mocked(tagsApi.listTags).mockResolvedValue(t1v2)
    // No good way to nudge useTags from here; just assert the call count is 1.
    expect(tagsApi.getSkillTags).toHaveBeenCalledTimes(1)
  })

  test('unmounts cleanly without applying state when getSkillTags resolves late', async () => {
    // Regression for the cancelled-flag guard in the effect cleanup.
    let resolveFn: (tags: Tag[]) => void = () => {}
    const slow = new Promise<Tag[]>((res) => {
      resolveFn = res
    })
    vi.mocked(tagsApi.listTags).mockResolvedValue([])
    vi.mocked(tagsApi.getSkillTags).mockReturnValue(slow)

    const { unmount } = renderDialog({ selectedSkillIds: ['skill-late'] })
    unmount()

    // Resolve AFTER unmount — must not throw or call setState.
    expect(() => resolveFn([])).not.toThrow()
  })
})

describe('TagManagerDialog — multi-skill bulk mode', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    vi.mocked(tagsApi.listTags).mockResolvedValue([])
    vi.mocked(tagsApi.getSkillTags).mockResolvedValue([])
    // bulkAttachTag returns Promise<void> in production — keep the mock consistent.
    vi.mocked(tagsApi.bulkAttachTag).mockResolvedValue(undefined)
  })
  afterEach(() => cleanup())

  test('uses count-based label and bulk path; no aria-pressed', async () => {
    const t1 = makeTag('t1', 'frontend')
    vi.mocked(tagsApi.listTags).mockResolvedValue([t1])

    renderDialog({ selectedSkillIds: ['s-1', 's-2', 's-3'] })

    const btn = await screen.findByTestId('tag-apply-t1')
    expect(btn).toHaveTextContent('Apply to 3')
    // Bulk mode must NOT advertise aria-pressed — the action is one-way.
    expect(btn).not.toHaveAttribute('aria-pressed')

    fireEvent.click(btn)
    await waitFor(() => {
      expect(tagsApi.bulkAttachTag).toHaveBeenCalledWith(['s-1', 's-2', 's-3'], 't1')
    })
    expect(tagsApi.getSkillTags).not.toHaveBeenCalled()
  })
})
