import { invokeCommand } from './index';

export async function syncSkills(skillNames?: string[]): Promise<number> {
  return invokeCommand<number>('sync_skills', {
    ...(skillNames !== undefined && { skillNames }),
  });
}

export async function cleanStaleLinks(): Promise<number> {
  return invokeCommand<number>('clean_stale_links');
}

export interface SyncProvider {
  id: string;
  name: string;
  kind: 'webdav' | 's3' | 'sftp';
  config_json: string;
  enabled: boolean;
  last_sync_at: string | null;
  last_sync_status: string | null;
  last_sync_error: string | null;
  created_at: string;
}

export interface SyncHistoryEntry {
  id: string;
  provider_id: string;
  direction: 'upload' | 'download' | 'bisync';
  status: 'pending' | 'running' | 'success' | 'failed' | 'partial';
  started_at: string;
  finished_at: string | null;
  bytes_transferred: number | null;
  skills_count: number | null;
  error_message: string | null;
}

export interface SyncPlan {
  provider_id: string;
  provider_name: string;
  actions: SyncPlanAction[];
  stats: SyncPlanStats;
}

export interface SyncPlanAction {
  skill_name: string;
  action_type: 'upload-local' | 'download-remote' | 'conflict' | 'delete-local' | 'delete-remote' | 'skip';
  local_mtime: string | null;
  remote_mtime: string | null;
  reason: string;
}

export interface SyncPlanStats {
  upload_count: number;
  download_count: number;
  conflict_count: number;
  delete_local_count: number;
  delete_remote_count: number;
  skip_count: number;
}

export interface SyncResult {
  provider_id: string;
  direction: 'upload' | 'download' | 'bisync';
  status: 'pending' | 'running' | 'success' | 'failed' | 'partial';
  started_at: string;
  finished_at: string | null;
  bytes_transferred: number;
  skills_synced: number;
  errors: string[];
}

export async function syncListProviders(): Promise<SyncProvider[]> {
  return invokeCommand<SyncProvider[]>('sync_list_providers');
}

export async function syncAddProvider(
  id: string,
  name: string,
  kind: string,
  configJson: string,
): Promise<SyncProvider> {
  return invokeCommand<SyncProvider>('sync_add_provider', { id, name, kind, configJson });
}

export async function syncDeleteProvider(id: string): Promise<void> {
  return invokeCommand<void>('sync_delete_provider', { id });
}

export async function syncTestConnection(id: string): Promise<void> {
  return invokeCommand<void>('sync_test_connection', { id });
}

export async function syncStart(id: string, direction: string): Promise<SyncResult> {
  return invokeCommand<SyncResult>('sync_start', { id, direction });
}

export async function syncGetPlan(id: string): Promise<SyncPlan> {
  return invokeCommand<SyncPlan>('sync_get_plan', { id });
}

export async function syncGetHistory(id: string, limit?: number): Promise<SyncHistoryEntry[]> {
  return invokeCommand<SyncHistoryEntry[]>('sync_get_history', { id, limit });
}

export interface RcloneStatus {
  installed: boolean;
  path: string | null;
  downloadUrl?: string;
}

export async function syncRcloneStatus(): Promise<RcloneStatus> {
  return invokeCommand<RcloneStatus>('sync_rclone_status');
}

export async function syncEnsureRclone(): Promise<RcloneStatus> {
  return invokeCommand<RcloneStatus>('sync_ensure_rclone');
}