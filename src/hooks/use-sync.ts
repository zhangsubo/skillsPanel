import { useCallback, useEffect, useState } from 'react';
import {
  createSyncProvider,
  deleteSyncProvider,
  getAllSyncHistory,
  getSyncHistory,
  listSyncProviders,
  syncNow,
  testSyncProviderConnection,
  updateSyncProvider,
} from '@/api/sync';
import type { SyncHistory, SyncProvider } from '@/types';

interface UseSyncState {
  providers: SyncProvider[];
  history: SyncHistory[];
  loading: boolean;
  error: Error | null;
  refresh: () => Promise<void>;
  create: (name: string, kind: string, configJson: string) => Promise<SyncProvider>;
  update: (
    id: string,
    fields: { name?: string; configJson?: string; enabled?: boolean },
  ) => Promise<void>;
  remove: (id: string) => Promise<void>;
  testConnection: (id: string) => Promise<void>;
  syncUp: (id: string) => Promise<SyncHistory>;
  syncDown: (id: string) => Promise<SyncHistory>;
  loadHistory: (providerId: string) => Promise<SyncHistory[]>;
}

/**
 * Hook for the cloud sync feature. Manages the global provider list
 * and recent history. Per-provider history (lazy) is fetched on
 * demand via `loadHistory` to avoid pulling the entire history table
 * for a Settings page that only shows summary cards.
 */
export function useSync(): UseSyncState {
  const [providers, setProviders] = useState<SyncProvider[]>([]);
  const [history, setHistory] = useState<SyncHistory[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<Error | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [list, recent] = await Promise.all([
        listSyncProviders(),
        getAllSyncHistory(20),
      ]);
      setProviders(list);
      setHistory(recent);
    } catch (e) {
      setError(e as Error);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const create = useCallback(
    async (name: string, kind: string, configJson: string) => {
      const provider = await createSyncProvider(name, kind, configJson);
      setProviders((prev) => [...prev, provider].sort((a, b) => a.name.localeCompare(b.name)));
      return provider;
    },
    [],
  );

  const update = useCallback(
    async (id: string, fields: { name?: string; configJson?: string; enabled?: boolean }) => {
      await updateSyncProvider(id, fields);
      setProviders((prev) =>
        prev.map((p) =>
          p.id === id
            ? {
                ...p,
                ...(fields.name !== undefined && { name: fields.name }),
                ...(fields.configJson !== undefined && { config_json: fields.configJson }),
                ...(fields.enabled !== undefined && { enabled: fields.enabled }),
              }
            : p,
        ),
      );
    },
    [],
  );

  const remove = useCallback(async (id: string) => {
    await deleteSyncProvider(id);
    setProviders((prev) => prev.filter((p) => p.id !== id));
    setHistory((prev) => prev.filter((h) => h.provider_id !== id));
  }, []);

  const testConnection = useCallback(async (id: string) => {
    await testSyncProviderConnection(id);
    await refresh();
  }, [refresh]);

  const syncUp = useCallback(
    async (id: string) => {
      const h = await syncNow(id, 'upload');
      setHistory((prev) => [h, ...prev].slice(0, 50));
      await refresh();
      return h;
    },
    [refresh],
  );

  const syncDown = useCallback(
    async (id: string) => {
      const h = await syncNow(id, 'download');
      setHistory((prev) => [h, ...prev].slice(0, 50));
      await refresh();
      return h;
    },
    [refresh],
  );

  const loadHistory = useCallback(async (providerId: string) => {
    return getSyncHistory(providerId, 50);
  }, []);

  return {
    providers,
    history,
    loading,
    error,
    refresh,
    create,
    update,
    remove,
    testConnection,
    syncUp,
    syncDown,
    loadHistory,
  };
}
