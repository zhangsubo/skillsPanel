import { describe, test, expect, vi, beforeEach } from 'vitest'

// Mock the Tauri shim before importing the module under test.
vi.mock('../index', () => ({
  invokeCommand: vi.fn(),
}))

import { invokeCommand } from '../index'
import {
  listSyncProviders,
  createSyncProvider,
  updateSyncProvider,
  deleteSyncProvider,
  getSyncHistory,
  getAllSyncHistory,
  testSyncProviderConnection,
  syncNow,
} from '../sync'

describe('sync API contracts', () => {
  beforeEach(() => {
    vi.mocked(invokeCommand).mockReset()
  })

  test('listSyncProviders invokes list_sync_providers with no args', async () => {
    vi.mocked(invokeCommand).mockResolvedValue([])
    await listSyncProviders()
    expect(invokeCommand).toHaveBeenCalledWith('list_sync_providers')
  })

  test('createSyncProvider passes name/kind/configJson verbatim', async () => {
    vi.mocked(invokeCommand).mockResolvedValue({
      id: 'p1',
      name: 'personal',
      kind: 'webdav',
      config_json: '{}',
      enabled: true,
      last_sync_at: null,
      last_sync_status: null,
      last_sync_error: null,
      created_at: '2024-01-01T00:00:00Z',
    })
    const result = await createSyncProvider('personal', 'webdav', '{"url":"x"}')
    expect(result.id).toBe('p1')
    expect(invokeCommand).toHaveBeenCalledWith('create_sync_provider', {
      name: 'personal',
      kind: 'webdav',
      configJson: '{"url":"x"}',
    })
  })

  test('updateSyncProvider only includes defined fields (preserves leave/clear semantics)', async () => {
    vi.mocked(invokeCommand).mockResolvedValue(undefined)
    await updateSyncProvider('p1', { name: 'renamed', enabled: false })
    expect(invokeCommand).toHaveBeenCalledWith('update_sync_provider', {
      id: 'p1',
      name: 'renamed',
      enabled: false,
    })
  })

  test('updateSyncProvider with empty fields sends only id', async () => {
    vi.mocked(invokeCommand).mockResolvedValue(undefined)
    await updateSyncProvider('p1', {})
    expect(invokeCommand).toHaveBeenCalledWith('update_sync_provider', {
      id: 'p1',
    })
  })

  test('deleteSyncProvider sends id', async () => {
    vi.mocked(invokeCommand).mockResolvedValue(undefined)
    await deleteSyncProvider('p1')
    expect(invokeCommand).toHaveBeenCalledWith('delete_sync_provider', { id: 'p1' })
  })

  test('getSyncHistory passes providerId and optional limit', async () => {
    vi.mocked(invokeCommand).mockResolvedValue([])
    await getSyncHistory('p1', 5)
    expect(invokeCommand).toHaveBeenCalledWith('get_sync_history', {
      providerId: 'p1',
      limit: 5,
    })
  })

  test('getSyncHistory without limit omits the field', async () => {
    vi.mocked(invokeCommand).mockResolvedValue([])
    await getSyncHistory('p1')
    expect(invokeCommand).toHaveBeenCalledWith('get_sync_history', {
      providerId: 'p1',
    })
  })

  test('getAllSyncHistory limit is optional', async () => {
    vi.mocked(invokeCommand).mockResolvedValue([])
    await getAllSyncHistory()
    expect(invokeCommand).toHaveBeenCalledWith('get_all_sync_history', {})
  })

  test('testSyncProviderConnection sends providerId', async () => {
    vi.mocked(invokeCommand).mockResolvedValue(undefined)
    await testSyncProviderConnection('p1')
    expect(invokeCommand).toHaveBeenCalledWith('test_sync_provider_connection', {
      providerId: 'p1',
    })
  })

  test('syncNow passes direction enum as a plain string', async () => {
    vi.mocked(invokeCommand).mockResolvedValue({
      id: 'h1',
      provider_id: 'p1',
      direction: 'upload',
      status: 'success',
      started_at: '2024-01-01T00:00:00Z',
      finished_at: '2024-01-01T00:00:01Z',
      bytes_transferred: 1024,
      skills_count: 3,
      error_message: null,
    })
    const h = await syncNow('p1', 'download')
    expect(h.direction).toBe('upload') // mock returned 'upload' but the arg is what matters
    expect(invokeCommand).toHaveBeenCalledWith('sync_now', {
      providerId: 'p1',
      direction: 'download',
    })
  })
})
