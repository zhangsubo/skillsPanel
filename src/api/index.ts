import { invoke } from '@tauri-apps/api/core';
import type { Tag } from '@/types';

function isTauriEnv(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

const MOCK_COMMANDS: Record<string, (args?: Record<string, unknown>) => unknown> = {
  get_library: () => [],
  get_tools: () => [
    { id: 'cursor', name: 'cursor', path: '/Users/demo/.cursor', enabled: true, is_custom: false },
    { id: 'claude', name: 'claude', path: '/Users/demo/.claude', enabled: true, is_custom: false },
    { id: 'trae', name: 'trae', path: '/Users/demo/.trae', enabled: true, is_custom: false },
    { id: 'copilot', name: 'copilot', path: '/Users/demo/.github-copilot', enabled: true, is_custom: false },
    { id: 'codex', name: 'codex', path: '/Users/demo/.codex', enabled: false, is_custom: false },
  ],
  get_config: () => JSON.stringify({
    library_path: '~/.skills-panel/skills',
    tools: [
      { id: 'opencode', name: 'OpenCode', path: '~/.config/opencode/skill', enabled: true, is_custom: false },
      { id: 'antigravity', name: 'Antigravity', path: '~/.antigravity/skills', enabled: true, is_custom: false },
      { id: 'codex', name: 'Codex', path: '~/.codex/skills', enabled: true, is_custom: false },
      { id: 'trae', name: 'Trae', path: '~/.trae/skills', enabled: true, is_custom: false },
      { id: 'gemini-cli', name: 'Gemini CLI', path: '~/.gemini/skills', enabled: true, is_custom: false },
      { id: 'hermes', name: 'Hermes', path: '~/.hermes/skills', enabled: true, is_custom: false },
      { id: 'openclaw', name: 'OpenClaw', path: '~/.openclaw/skills', enabled: false, is_custom: false },
    ],
    sources: [],
    sync: { mode: 'symlink' },
    install: { allow_zip: true, allow_git: true, default_sync_targets: [] },
    exclude_paths: ['node_modules', '.git', 'dist', 'coverage'],
    rules: { tools: {}, groups: {}, skills: {} },
    deleted_skills: [],
    debug_logging: false,
  }),
  get_audit_logs: () => [],
  scan_skills: () => {
    const mockSkills = [
      { name: 'file-organizer', desc: '自动整理和分类文件，支持按日期、类型、标签等多种规则组织文件系统', paths: ['/Users/demo/.hermes/skills/productivity/file-organizer'] },
      { name: 'git-helper', desc: 'Git 工作流自动化工具，支持分支管理、提交规范检查和冲突解决建议', paths: ['/Users/demo/.hermes/skills/devops/git-helper', '/Users/demo/.trae/skills/git-helper'] },
      { name: 'code-reviewer', desc: '智能代码审查助手，基于 AST 分析提供代码质量建议和潜在 Bug 检测', paths: ['/Users/demo/.hermes/skills/dev/code-reviewer'] },
      { name: 'airtable-sync', desc: 'Airtable 数据同步工具，支持双向同步、字段映射和自动冲突解决', paths: ['/Users/demo/.hermes/skills/productivity/airtable-sync'] },
      { name: 'apple-notes', desc: '苹果备忘录集成工具，支持读取、创建和搜索备忘录内容', paths: ['/Users/demo/.hermes/skills/productivity/apple-notes', '/Users/demo/.cursor/skills/apple-notes'] },
      { name: 'slack-bot', desc: 'Slack 机器人框架，支持消息发送、频道管理和 webhook 处理', paths: ['/Users/demo/.hermes/skills/communication/slack-bot'] },
      { name: 'discord-webhook', desc: 'Discord Webhook 管理工具，支持消息模板、富文本和嵌入消息发送', paths: ['/Users/demo/.hermes/skills/communication/discord-webhook'] },
      { name: 'github-actions', desc: 'GitHub Actions 工作流生成器，支持 CI/CD 模板和自定义 action 组合', paths: ['/Users/demo/.hermes/skills/devops/github-actions', '/Users/demo/.trae/skills/github-actions'] },
      { name: 'docker-compose', desc: 'Docker Compose 配置管理工具，支持多环境配置和一键启动脚本生成', paths: ['/Users/demo/.hermes/skills/devops/docker-compose'] },
      { name: 'kubernetes-helper', desc: 'Kubernetes 集群管理助手，支持 Pod 查看、日志收集和资源监控', paths: ['/Users/demo/.hermes/skills/devops/kubernetes-helper'] },
      { name: 'aws-cli', desc: 'AWS CLI 封装工具，提供常用 S3、EC2、Lambda 操作的快捷命令', paths: ['/Users/demo/.hermes/skills/cloud/aws-cli'] },
      { name: 'terraform-tool', desc: 'Terraform 基础设施即代码助手，支持状态管理和模块依赖分析', paths: ['/Users/demo/.hermes/skills/cloud/terraform-tool'] },
      { name: 'nginx-config', desc: 'Nginx 配置生成器和验证工具，支持反向代理、负载均衡和 SSL 配置', paths: ['/Users/demo/.hermes/skills/devops/nginx-config'] },
      { name: 'mysql-backup', desc: 'MySQL 数据库备份和恢复工具，支持增量备份、定时任务和云存储同步', paths: ['/Users/demo/.hermes/skills/database/mysql-backup'] },
      { name: 'redis-manager', desc: 'Redis 缓存管理工具，支持键值查看、内存分析和性能监控', paths: ['/Users/demo/.hermes/skills/database/redis-manager'] },
      { name: 'elasticsearch-query', desc: 'Elasticsearch 查询构建器，支持 DSL 生成、聚合分析和索引管理', paths: ['/Users/demo/.hermes/skills/database/elasticsearch-query'] },
      { name: 'prometheus-alert', desc: 'Prometheus 告警规则管理工具，支持告警模板和通知渠道配置', paths: ['/Users/demo/.hermes/skills/monitoring/prometheus-alert'] },
      { name: 'grafana-dashboard', desc: 'Grafana 仪表盘生成器，支持多种数据源和可视化面板模板', paths: ['/Users/demo/.hermes/skills/monitoring/grafana-dashboard'] },
      { name: 'jenkins-pipeline', desc: 'Jenkins Pipeline 脚本生成器，支持多分支流水线和并行任务配置', paths: ['/Users/demo/.hermes/skills/devops/jenkins-pipeline'] },
      { name: 'gitlab-ci', desc: 'GitLab CI/CD 配置助手，支持模板库和跨项目流水线触发', paths: ['/Users/demo/.hermes/skills/devops/gitlab-ci'] },
      { name: 'npm-publish', desc: 'NPM 包发布助手，支持版本管理、标签发布和私有仓库配置', paths: ['/Users/demo/.hermes/skills/package/npm-publish'] },
      { name: 'webpack-config', desc: 'Webpack 配置优化工具，支持性能分析、代码分割和懒加载策略', paths: ['/Users/demo/.hermes/skills/build/webpack-config'] },
      { name: 'jest-test', desc: 'Jest 测试框架助手，支持快照测试、覆盖率报告和并行执行优化', paths: ['/Users/demo/.hermes/skills/testing/jest-test'] },
      { name: 'eslint-plugin', desc: 'ESLint 规则配置工具，支持自定义规则集和自动修复建议', paths: ['/Users/demo/.hermes/skills/lint/eslint-plugin'] },
      { name: 'prettier-format', desc: 'Prettier 格式化配置管理器，支持多语言和团队统一风格配置', paths: ['/Users/demo/.hermes/skills/lint/prettier-format'] },
      { name: 'typescript-check', desc: 'TypeScript 类型检查助手，支持严格模式配置和类型推断优化', paths: ['/Users/demo/.hermes/skills/lint/typescript-check'] },
      { name: 'markdown-render', desc: 'Markdown 渲染和转换工具，支持表格、图表和数学公式渲染', paths: ['/Users/demo/.hermes/skills/doc/markdown-render'] },
      { name: 'pdf-generator', desc: 'PDF 文档生成器，支持模板引擎、页眉页脚和水印添加', paths: ['/Users/demo/.hermes/skills/doc/pdf-generator'] },
      { name: 'image-compress', desc: '图片压缩优化工具，支持多种格式和批量处理，保持视觉质量', paths: ['/Users/demo/.hermes/skills/media/image-compress'] },
      { name: 'video-convert', desc: '视频格式转换器，支持主流编解码器和分辨率自适应调整', paths: ['/Users/demo/.hermes/skills/media/video-convert'] },
    ];

    const toolNames = ['cursor', 'claude', 'trae', 'copilot', 'codex'];

    const skills = mockSkills.map((skill, index) => {
      const linkedCount = Math.min(Math.floor(Math.random() * 4) + 1, toolNames.length);
      const statuses: Record<string, string> = {};
      const shuffled = [...toolNames].sort(() => Math.random() - 0.5);
      for (let i = 0; i < linkedCount; i++) {
        statuses[shuffled[i]] = 'linked';
      }

      return {
        skill: {
          id: `skill-${index + 1}`,
          name: skill.name,
          path_hash: `hash${index + 1}`,
          library_path: skill.paths[0],
          original_source_path: skill.paths.length > 1 ? skill.paths[1] : null,
          original_git_url: null,
          original_git_subpath: null,
          group: 'default',
          description: skill.desc,
          frontmatter: {},
          created_at: new Date().toISOString(),
          mtime_ms: Date.now(),
          source_type: 'local-folder' as const,
          is_deleted: false,
          source_revision: null,
          source_remote_revision: null,
          source_update_status: 'up-to-date' as const,
        },
        tool_statuses: statuses,
        rule_decisions: {},
      };
    });

    return {
      skills,
      total_skills: skills.length,
      total_tools: 5,
      linked_count: 12,
      conflict_count: 3,
      blocked_count: 2,
    };
  },
  get_skill_content: () => '# Skill\n\nMock content for development.',
  preview_local_install: () => [
    {
      candidate_id: 'mock-1',
      detected_name: 'file-organizer',
      user_name_override: null,
      description: 'A skill for organizing files automatically',
      source_path: '/Users/demo/skills/file-organizer',
      skill_root: '.',
      valid: true,
      error: null,
    },
    {
      candidate_id: 'mock-2',
      detected_name: 'git-helper',
      user_name_override: null,
      description: 'Git workflow automation skill',
      source_path: '/Users/demo/skills/git-helper',
      skill_root: '.',
      valid: true,
      error: null,
    },
    {
      candidate_id: 'mock-3',
      detected_name: 'code-reviewer',
      user_name_override: null,
      description: null,
      source_path: '/Users/demo/skills/code-reviewer',
      skill_root: '.',
      valid: false,
      error: 'Missing SKILL.md',
    },
  ],
  get_app_version: () => '0.2.6',
  log_message: () => undefined,
  get_app_logs: () => [],
  install_local_skill: () => '/mock/library/file-organizer',
  preview_git_install: () => [
    {
      candidate_id: 'git-mock-1',
      detected_name: 'example-skill',
      user_name_override: null,
      description: 'An example skill from git',
      source_path: 'https://github.com/example/repo',
      skill_root: '.',
      valid: true,
      error: null,
    },
  ],
  install_git_skill: () => '/mock/library/example-skill',
  check_skill_update: () => false,
  update_skill: () => '/mock/library/updated-skill',
  cancel_install: () => undefined,
  link_skill: () => undefined,
  unlink_skill: () => undefined,
  delete_skill: () => undefined,
  batch_delete_skills: (args) => {
    const names = (args as Record<string, unknown>)?.skillNames as string[] | undefined;
    return names?.length ?? 0;
  },
  update_config: () => undefined,
  get_scan_diff: () => ({ added: [], removed: [], updated: [] }),
  sync_skills: () => undefined,
  export_skill: () => undefined,
  batch_export_skills: () => 0,
  batch_link_skills: () => 0,
  batch_unlink_skills: () => 0,
  clean_stale_links: () => 0,
  fix_skill_link: () => undefined,
  restore_skill: () => undefined,
  get_config_value: () => null,
  set_config_value: () => undefined,
  update_group_rule: () => undefined,
  update_skill_rule: () => undefined,
  update_tool_rule: () => undefined,
  mark_skill_installed: () => undefined,
  mark_skill_uninstalled: () => undefined,
  upsert_skill_to_db: () => undefined,
  upsert_tool_to_db: () => undefined,
  get_all_active_skills_from_db: () => [],
  get_installed_skills_from_db: () => [],
  get_linked_tool_ids: () => [],
  get_tools_from_db: () => [],
  get_audit_logs_from_db: () => [],
  get_app_logs_from_db: () => [],
  log_app_message: () => undefined,
  log_audit_entry: () => undefined,

  // ── Project / Workspace Mocks ──────────────────────────────────
  list_projects: () => [
    { id: 'p1', name: 'skillsPanel', root_path: '/Users/demo/Code/skillsPanel', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
    { id: 'p2', name: 'my-web-app', root_path: '/Users/demo/Code/my-web-app', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
  ],
  create_project: () => undefined,
  delete_project: () => undefined,
  scan_project: () => ({
    project: { id: 'p1', name: 'skillsPanel', root_path: '/Users/demo/Code/skillsPanel', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
    skills: [
      { name: 'code-reviewer', description: '智能代码审查助手', relative_path: 'code-reviewer', agent: 'claude-code', enabled: true, content_hash: 'abc123', in_center: true, center_skill_id: 's1', sync_status: 'in_sync' },
      { name: 'git-helper', description: 'Git 工作流自动化工具', relative_path: 'git-helper', agent: 'cursor', enabled: true, content_hash: 'def456', in_center: true, center_skill_id: 's2', sync_status: 'diverged' },
      { name: 'project-only-skill', description: '仅在项目中存在的技能', relative_path: 'project-only-skill', agent: 'claude-code', enabled: false, content_hash: null, in_center: false, center_skill_id: null, sync_status: 'project_only' },
    ],
    sync_health: { in_sync: 1, center_newer: 0, project_newer: 0, diverged: 1, project_only: 1, center_only: 0 },
  }),
  import_project_skill: () => '/mock/library/imported-skill',
  export_skill_to_project: () => undefined,
};

// ── Tag Mocks (browser-only, persistent across invocations) ────────
// 内存 state 跨命令持久化，方便浏览器模式下做交互联调。
let mockTagSeq = 0;
const mockTags: Tag[] = [
  { id: 't-rust', name: 'rust', color: '#dea584', description: 'Rust lang skills', created_at: '2024-01-01T00:00:00Z' },
  { id: 't-frontend', name: 'frontend', color: '#3b82f6', description: null, created_at: '2024-01-02T00:00:00Z' },
];
const mockSkillTagLinks: Array<{ skill_id: string; tag_id: string }> = [
  { skill_id: 's1', tag_id: 't-rust' },
];

const MOCK_TAG_COMMANDS: Record<string, (args?: Record<string, unknown>) => unknown> = {
  list_tags: () => mockTags.slice().sort((a, b) => a.name.localeCompare(b.name)),
  create_tag: (args) => {
    const a = args as { name: string; color?: string | null; description?: string | null };
    if (mockTags.some((t) => t.name === a.name)) {
      throw new Error(`Tag already exists: ${a.name}`);
    }
    mockTagSeq += 1;
    const tag: Tag = {
      id: `t-mock-${mockTagSeq}`,
      name: a.name,
      color: a.color ?? null,
      description: a.description ?? null,
      created_at: new Date().toISOString(),
    };
    mockTags.push(tag);
    return tag;
  },
  update_tag: (args) => {
    const a = args as { id: string; name?: string | null; color?: string | null; description?: string | null };
    const tag = mockTags.find((t) => t.id === a.id);
    if (!tag) throw new Error(`Tag not found: ${a.id}`);
    if (a.name !== null && a.name !== undefined) tag.name = a.name;
    if (a.color !== null && a.color !== undefined) tag.color = a.color;
    if (a.description !== null && a.description !== undefined) tag.description = a.description || null;
  },
  delete_tag: (args) => {
    const a = args as { id: string };
    const idx = mockTags.findIndex((t) => t.id === a.id);
    if (idx >= 0) mockTags.splice(idx, 1);
    for (let i = mockSkillTagLinks.length - 1; i >= 0; i--) {
      if (mockSkillTagLinks[i].tag_id === a.id) mockSkillTagLinks.splice(i, 1);
    }
  },
  attach_tag: (args) => {
    const a = args as { skillId: string; tagId: string };
    if (!mockSkillTagLinks.some((l) => l.skill_id === a.skillId && l.tag_id === a.tagId)) {
      mockSkillTagLinks.push({ skill_id: a.skillId, tag_id: a.tagId });
    }
  },
  detach_tag: (args) => {
    const a = args as { skillId: string; tagId: string };
    for (let i = mockSkillTagLinks.length - 1; i >= 0; i--) {
      if (mockSkillTagLinks[i].skill_id === a.skillId && mockSkillTagLinks[i].tag_id === a.tagId) {
        mockSkillTagLinks.splice(i, 1);
      }
    }
  },
  bulk_attach_tag: (args) => {
    const a = args as { skillIds: string[]; tagId: string };
    for (const skillId of a.skillIds) {
      if (!mockSkillTagLinks.some((l) => l.skill_id === skillId && l.tag_id === a.tagId)) {
        mockSkillTagLinks.push({ skill_id: skillId, tag_id: a.tagId });
      }
    }
  },
  get_skill_tags: (args) => {
    const a = args as { skillId: string };
    return mockTags.filter((t) => mockSkillTagLinks.some((l) => l.skill_id === a.skillId && l.tag_id === t.id));
  },
  get_all_skill_tags: () =>
    mockSkillTagLinks
      .map((l) => {
        const tag = mockTags.find((t) => t.id === l.tag_id);
        return tag ? ([l.skill_id, tag] as [string, Tag]) : null;
      })
      .filter((x): x is [string, Tag] => x !== null),
};

// 把 tag mocks 合并到主 MOCK_COMMANDS 里。
for (const [cmd, handler] of Object.entries(MOCK_TAG_COMMANDS)) {
  MOCK_COMMANDS[cmd] = handler;
}

// ── Cloud sync mocks (browser mode) ────────────────────────────────
// In-memory fixtures so `npm run dev` works without Tauri.

const MOCK_SYNC_PROVIDERS: Array<{
  id: string;
  name: string;
  kind: string;
  config_json: string;
  enabled: boolean;
  last_sync_at: string | null;
  last_sync_status: string | null;
  last_sync_error: string | null;
  created_at: string;
}> = [];

const MOCK_SYNC_HISTORY: Array<{
  id: string;
  provider_id: string;
  direction: string;
  status: string;
  started_at: string;
  finished_at: string | null;
  bytes_transferred: number | null;
  skills_count: number | null;
  error_message: string | null;
}> = [];

const MOCK_SYNC_COMMANDS: Record<string, (args?: Record<string, unknown>) => unknown> = {
  list_sync_providers: () => MOCK_SYNC_PROVIDERS.slice(),
  create_sync_provider: (args) => {
    const id = crypto.randomUUID();
    const provider = {
      id,
      name: String(args?.name ?? ''),
      kind: String(args?.kind ?? ''),
      config_json: String(args?.configJson ?? '{}'),
      enabled: true,
      last_sync_at: null,
      last_sync_status: null,
      last_sync_error: null,
      created_at: new Date().toISOString(),
    };
    MOCK_SYNC_PROVIDERS.push(provider);
    return provider;
  },
  update_sync_provider: (args) => {
    const id = String(args?.id ?? '');
    const provider = MOCK_SYNC_PROVIDERS.find((p) => p.id === id);
    if (!provider) throw new Error(`Provider ${id} not found`);
    if (typeof args?.name === 'string') provider.name = args.name;
    if (typeof args?.configJson === 'string') provider.config_json = args.configJson;
    if (typeof args?.enabled === 'boolean') provider.enabled = args.enabled;
    return undefined;
  },
  delete_sync_provider: (args) => {
    const id = String(args?.id ?? '');
    const idx = MOCK_SYNC_PROVIDERS.findIndex((p) => p.id === id);
    if (idx < 0) throw new Error(`Provider ${id} not found`);
    MOCK_SYNC_PROVIDERS.splice(idx, 1);
    // Cascade: drop history rows for this provider.
    for (let i = MOCK_SYNC_HISTORY.length - 1; i >= 0; i--) {
      if (MOCK_SYNC_HISTORY[i].provider_id === id) MOCK_SYNC_HISTORY.splice(i, 1);
    }
    return undefined;
  },
  get_sync_history: (args) => {
    const pid = String(args?.providerId ?? '');
    const limit = Number(args?.limit ?? 20);
    return MOCK_SYNC_HISTORY
      .filter((h) => h.provider_id === pid)
      .sort((a, b) => b.started_at.localeCompare(a.started_at))
      .slice(0, limit);
  },
  get_all_sync_history: (args) => {
    const limit = Number(args?.limit ?? 20);
    return MOCK_SYNC_HISTORY
      .slice()
      .sort((a, b) => b.started_at.localeCompare(a.started_at))
      .slice(0, limit);
  },
  test_sync_provider_connection: () => undefined,
  sync_now: (args) => {
    const pid = String(args?.providerId ?? '');
    const history = {
      id: crypto.randomUUID(),
      provider_id: pid,
      direction: String(args?.direction ?? 'upload'),
      status: 'success',
      started_at: new Date().toISOString(),
      finished_at: new Date().toISOString(),
      bytes_transferred: 0,
      skills_count: 0,
      error_message: null,
    };
    MOCK_SYNC_HISTORY.push(history);
    return history;
  },
};

for (const [cmd, handler] of Object.entries(MOCK_SYNC_COMMANDS)) {
  MOCK_COMMANDS[cmd] = handler;
}

function logToBackend(level: string, message: string, source: string): void {
  if (isTauriEnv()) {
    invoke('log_message', { level, message, source }).catch(() => {
      // Silently ignore log failures to avoid infinite loops
    });
  }
}

export async function invokeCommand<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!isTauriEnv()) {
    const mock = MOCK_COMMANDS[cmd];
    if (mock) {
      return Promise.resolve(mock(args) as T);
    }
    throw new Error(
      `Tauri command "${cmd}" is not available in browser mode. ` +
        `Please run the app via "npm run tauri dev" for full functionality.`,
    );
  }

  logToBackend('debug', `invoke ${cmd} args=${JSON.stringify(args)}`, 'frontend:invoke');

  try {
    const result = await invoke<T>(cmd, args);
    logToBackend('debug', `invoke ${cmd} OK`, 'frontend:invoke');
    return result;
  } catch (error) {
    const message =
      error instanceof Error
        ? error.message
        : typeof error === 'string'
          ? error
          : String(error);
    logToBackend('error', `invoke ${cmd} failed: ${message}`, 'frontend:invoke');
    throw new Error(`Tauri command "${cmd}" failed: ${message}`);
  }
}

export function toSnakeCase(args: Record<string, unknown>): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(args)) {
    const snakeKey = key.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`);
    result[snakeKey] = value;
  }
  return result;
}