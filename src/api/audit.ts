import { invokeCommand } from './index';
import type { AuditEntry } from '@/types';

export async function getAuditLogs(limit?: number): Promise<AuditEntry[]> {
  return invokeCommand<AuditEntry[]>('get_audit_logs', {
    ...(limit !== undefined && { limit }),
  });
}