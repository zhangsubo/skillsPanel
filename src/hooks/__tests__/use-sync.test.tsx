import { describe, test, expect, vi, beforeEach } from 'vitest'
import { render, waitFor, act } from '@testing-library/react'
import { useEffect } from 'react'

// Mock the sync API before importing the hook.
vi.mock('@/api/sync', () => ({
  listSyncProviders: vi.fn(),
  createSyncProvider: vi.fn(),
  updateSyncProvider: vi.fn(),
  deleteSyncProvider: vi.fn(),
  getSyncHistory: vi.fn(),
  getAllSyncHistory: vi.fn(),
  testSyncProviderConnection: vi.fn(),
  syncNow: vi.fn(),
}))

import * as syncApi from '@/api/sync'
import { useSync } from '@/hooks/use-sync'
import type { SyncProvider, SyncHistory } from '@/types'

function makeProvider(overrides: Partial<SyncProvider> = {}): SyncProvider {
  return {
    id: 'p1',
    name: 'personal',
    kind: 'webdav',
    config_json: '{}',
    enabled: true,
    last_sync_at: null,
    last_sync_status: null,
    last_sync_error: null,
    created_at: '2024-01-01T00:00:00Z',
    ...overrides,
  }
}

function makeHistory(overrides: Partial<SyncHistory> = {}): SyncHistory {
  return {
    id: 'h1',
    provider_id: 'p1',
    direction: 'upload',
    status: 'success',
    started_at: '2024-01-01T00:00:00Z',
    finished_at: '2024-01-01T00:00:01Z',
    bytes_transferred: 1024,
    skills_count: 3,
    error_message: null,
    ...overrides,
  }
}

function Probe({ onChange }: { onChange: (state: ReturnType<typeof useSync>) => void }) {
  const state = useSync()
  useEffect(() => {
    onChange(state)
  })
  return null
}

let lastState: ReturnType<typeof useSync> | null = null

function renderHook() {
  return render(<Probe onChange={(s) => (lastState = s)} />)
}

describe('useSync', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    lastState = null
  })

  test('initial mount fetches providers + history', async () => {
    const providers = [makeProvider()]
    const history = [makeHistory()]
    vi.mocked(syncApi.listSyncProviders).mockResolvedValue(providers)
    vi.mocked(syncApi.getAllSyncHistory).mockResolvedValue(history)

    renderHook()

    await waitFor(() => {
      expect(lastState).not.toBeNull()
    })
    await waitFor(() => {
      expect(lastState?.providers).toEqual(providers)
    })
    expect(lastState?.history).toEqual(history)
    expect(lastState?.loading).toBe(false)
  })

  test('create() appends the new provider to local state', async () => {
    vi.mocked(syncApi.listSyncProviders).mockResolvedValue([])
    vi.mocked(syncApi.getAllSyncHistory).mockResolvedValue([])
    const newProvider = makeProvider({ id: 'p2', name: 'work' })
    vi.mocked(syncApi.createSyncProvider).mockResolvedValue(newProvider)

    renderHook()
    await waitFor(() => expect(lastState?.providers).toEqual([]))

    await act(async () => {
      await lastState!.create('work', 'webdav', '{}')
    })

    expect(lastState?.providers.map((p) => p.id)).toEqual(['p2'])
  })

  test('update() merges fields into the matching provider', async () => {
    const original = makeProvider({ name: 'old' })
    vi.mocked(syncApi.listSyncProviders).mockResolvedValue([original])
    vi.mocked(syncApi.getAllSyncHistory).mockResolvedValue([])

    renderHook()
    await waitFor(() => expect(lastState?.providers.length).toBe(1))

    await act(async () => {
      await lastState!.update('p1', { name: 'new' })
    })

    expect(lastState?.providers[0].name).toBe('new')
    expect(lastState?.providers[0].enabled).toBe(true) // unchanged
  })

  test('remove() drops provider AND its history', async () => {
    const p1 = makeProvider({ id: 'p1' })
    const p2 = makeProvider({ id: 'p2', name: 'work' })
    vi.mocked(syncApi.listSyncProviders).mockResolvedValue([p1, p2])
    vi.mocked(syncApi.getAllSyncHistory).mockResolvedValue([
      makeHistory({ provider_id: 'p1' }),
      makeHistory({ provider_id: 'p2' }),
    ])

    renderHook()
    await waitFor(() => expect(lastState?.providers.length).toBe(2))
    expect(lastState?.history.length).toBe(2)

    await act(async () => {
      await lastState!.remove('p1')
    })

    expect(lastState?.providers.map((p) => p.id)).toEqual(['p2'])
    expect(lastState?.history.every((h) => h.provider_id !== 'p1')).toBe(true)
  })

  test('syncUp() prepends new history and refreshes', async () => {
    vi.mocked(syncApi.listSyncProviders).mockResolvedValue([makeProvider()])
    vi.mocked(syncApi.getAllSyncHistory).mockResolvedValue([])
    const newHistory = makeHistory({ id: 'h-new' })
    vi.mocked(syncApi.syncNow).mockResolvedValue(newHistory)
    // After refresh, return the same history
    vi.mocked(syncApi.getAllSyncHistory)
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([newHistory])

    renderHook()
    await waitFor(() => expect(lastState?.providers.length).toBe(1))

    await act(async () => {
      await lastState!.syncUp('p1')
    })

    expect(lastState?.history[0].id).toBe('h-new')
  })
})
