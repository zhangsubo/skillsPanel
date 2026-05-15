import { invokeCommand } from './index';
import type { LogEntry } from '@/types';

export async function logMessage(
  level: string,
  message: string,
  source: string = 'frontend',
): Promise<void> {
  return invokeCommand<void>('log_message', { level, message, source });
}

export async function getAppLogs(limit?: number): Promise<LogEntry[]> {
  return invokeCommand<LogEntry[]>('get_app_logs', {
    ...(limit !== undefined && { limit }),
  });
}
