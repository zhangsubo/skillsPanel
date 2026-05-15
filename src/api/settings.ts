import { invokeCommand } from './index';

export interface ToolJson {
  id: string;
  name: string;
  path: string;
  enabled: boolean;
  is_custom: boolean;
}

export interface AppConfigJson {
  library_path: string;
  tools: ToolJson[];
  sources: Array<{
    path: string;
    group: string;
    default: boolean;
    enabled: boolean;
    recursive: boolean;
  }>;
  sync: { mode: string };
  install: {
    allow_zip: boolean;
    allow_git: boolean;
    default_sync_targets: string[];
  };
  exclude_paths: string[];
  rules: {
    tools: Record<string, unknown>;
    groups: Record<string, unknown>;
    skills: Record<string, unknown>;
  };
  deleted_skills: string[];
}

export async function getConfig(): Promise<AppConfigJson> {
  const raw = await invokeCommand<string>('get_config');
  return JSON.parse(raw);
}

export async function updateConfig(config: AppConfigJson): Promise<void> {
  return invokeCommand<void>('update_config', { configJson: JSON.stringify(config) });
}

export async function addCustomTool(
  name: string,
  path: string,
): Promise<void> {
  const config = await getConfig();
  const id = name.toLowerCase().replace(/\s+/g, '-');
  if (config.tools.some((t) => t.id === id)) {
    throw new Error(`Tool '${name}' already exists`);
  }
  config.tools.push({ id, name, path, enabled: true, is_custom: true });
  await updateConfig(config);
}

export async function removeTool(toolId: string): Promise<void> {
  const config = await getConfig();
  config.tools = config.tools.filter((t) => t.id !== toolId);
  await updateConfig(config);
}

export async function toggleToolEnabled(toolId: string, enabled: boolean): Promise<void> {
  const config = await getConfig();
  const tool = config.tools.find((t) => t.id === toolId);
  if (tool) {
    tool.enabled = enabled;
    await updateConfig(config);
  }
}
