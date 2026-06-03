import { invokeCommand } from './index';
import type { SyncHistory, SyncProvider } from '@/types';

export async function syncSkills(skillNames?: string[]): Promise<number> {
  return invokeCommand<number>('sync_skills', {
    ...(skillNames !== undefined && { skillNames }),
  });
}

export async function cleanStaleLinks(): Promise<number> {
  return invokeCommand<number>('clean_stale_links');
}

// ── Cloud sync (云备份) ──────────────────────────────────────────
// Thin wrappers around the 8 sync commands. All go through
// `invokeCommand` and resolve in browser mode via MOCK_SYNC_COMMANDS
// in `./index.ts`.

export async function listSyncProviders(): Promise<SyncProvider[]> {
  return invokeCommand<SyncProvider[]>('list_sync_providers');
}

export async function createSyncProvider(
  name: string,
  kind: string,
  configJson: string,
): Promise<SyncProvider> {
  return invokeCommand<SyncProvider>('create_sync_provider', {
    name,
    kind,
    configJson,
  });
}

export async function updateSyncProvider(
  id: string,
  fields: { name?: string; configJson?: string; enabled?: boolean },
): Promise<void> {
  return invokeCommand<void>('update_sync_provider', {
    id,
    ...(fields.name !== undefined && { name: fields.name }),
    ...(fields.configJson !== undefined && { configJson: fields.configJson }),
    ...(fields.enabled !== undefined && { enabled: fields.enabled }),
  });
}

export async function deleteSyncProvider(id: string): Promise<void> {
  return invokeCommand<void>('delete_sync_provider', { id });
}

export async function getSyncHistory(
  providerId: string,
  limit?: number,
): Promise<SyncHistory[]> {
  return invokeCommand<SyncHistory[]>('get_sync_history', {
    providerId,
    ...(limit !== undefined && { limit }),
  });
}

export async function getAllSyncHistory(
  limit?: number,
): Promise<SyncHistory[]> {
  return invokeCommand<SyncHistory[]>('get_all_sync_history', {
    ...(limit !== undefined && { limit }),
  });
}

export async function testSyncProviderConnection(
  providerId: string,
): Promise<void> {
  return invokeCommand<void>('test_sync_provider_connection', { providerId });
}

export async function syncNow(
  providerId: string,
  direction: 'upload' | 'download',
): Promise<SyncHistory> {
  return invokeCommand<SyncHistory>('sync_now', { providerId, direction });
}