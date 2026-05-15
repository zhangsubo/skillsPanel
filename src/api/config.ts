import { invokeCommand } from './index';
import type { AppConfig } from '@/types';

// Rust returns JSON string; parse here so callers get a typed object.
export async function getConfig(): Promise<AppConfig> {
  const json = await invokeCommand<string>('get_config');
  return JSON.parse(json) as AppConfig;
}

// Rust expects JSON string; stringify here so callers pass a typed object.
export async function updateConfig(config: AppConfig): Promise<void> {
  await invokeCommand<void>('update_config', {
    configJson: JSON.stringify(config),
  });
}