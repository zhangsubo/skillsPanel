# Skills Panel

Agent Skill Unified Management Tool — 统一管理 AI 编程工具的技能（Skills），支持本地、ZIP、Git 安装，技能同步，项目工作区管理。

## 功能

- **技能管理**：扫描本地技能目录，安装技能（本地文件夹/ZIP/Git），查看技能详情
- **多工具同步**：将技能通过 Symlink 或 Copy 模式同步到 Cursor、Claude Code、OpenCode、Codex 等多个 AI 工具
- **项目工作区**：扫描项目中的技能目录（`.claude/skills/`、`.cursor/skills/` 等），支持技能导入/导出中央仓库、同步健康状态检测
- **Git 安装增强**：Tree URL 解析、多技能仓库检测、克隆缓存、进度推送、取消机制
- **安全加固**：AES-256-GCM 敏感配置加密、RepoLock 安装互斥锁、内容哈希去重
- **配置统一**：数据库作为唯一配置源，JSON 文件仅作为备份/导出

## 技术栈

| 层 | 技术 |
|---|------|
| 前端 | React 19, TypeScript, Tailwind CSS, shadcn/ui |
| 后端 | Rust, Tauri 2, SQLite (rusqlite) |
| 代码分析 | [GitNexus](https://github.com/abhigyanpatwari/GitNexus), [Understand-Anything](https://understand-anything.com/) |

## 快速开始

```bash
# 安装依赖
npm install

# 开发模式
npm run tauri dev

# 构建
npm run tauri build
```

## 项目结构

```
.
├── src/                          # 前端代码
│   ├── pages/                    # 页面组件
│   │   ├── Dashboard.tsx         # 概览仪表板
│   │   ├── Library.tsx           # 中央仓库技能管理
│   │   ├── Scanner.tsx           # 技能安装（本地/Git）
│   │   ├── Settings.tsx          # 设置
│   │   ├── Tools.tsx             # 工具管理
│   │   └── SkillDetail.tsx       # 技能详情
│   ├── components/               # 通用组件
│   ├── hooks/                    # 自定义 React Hooks
│   ├── i18n/                     # 国际化（中/英）
│   └── api/                      # API 调用层
├── src-tauri/                    # 后端代码
│   ├── src/
│   │   ├── core/
│   │   │   ├── database.rs       # 数据库 schema + 迁移 + Repositories
│   │   │   ├── models.rs         # 数据模型
│   │   │   ├── config.rs         # 配置管理
│   │   │   ├── skill_engine.rs   # 技能安装引擎
│   │   │   ├── scanner.rs        # 本地技能扫描
│   │   │   ├── linker.rs         # Symlink/Copy 同步引擎
│   │   │   ├── library.rs        # 中央仓库管理
│   │   │   ├── installer.rs      # 安装逻辑
│   │   │   ├── resolver.rs       # 规则解析
│   │   │   ├── project_scanner.rs # 项目工作区扫描
│   │   │   ├── git_url_parser.rs # Git URL 解析
│   │   │   ├── install_cancel.rs # 取消令牌注册表
│   │   │   ├── repo_lock.rs      # 安装互斥锁
│   │   │   ├── crypto.rs         # AES-256-GCM 加密
│   │   │   ├── content_hash.rs   # 目录内容哈希
│   │   │   ├── fs_utils.rs       # 通用工具函数
│   │   │   ├── platform_fs.rs    # 跨平台文件系统
│   │   │   └── ...               # migration, audit, conflict 等
│   │   ├── commands.rs           # Tauri 命令
│   │   └── lib.rs                # 应用入口
│   ├── Cargo.toml
│   └── tauri.conf.json
├── assets/                       # 静态资源（图片、测试数据）
├── docs/                         # 项目文档
│   ├── agent-skill-manager-prd.md
│   ├── database-design.md
│   ├── sqlite-schema.sql
│   └── ...
├── index.html                    # HTML 入口
├── package.json
├── vite.config.ts
├── tsconfig.json
└── tailwind.config.js
```

## 数据流

```
用户操作 → Tauri Command → Core Module → SQLite DB
                              ↓
                         文件系统 (中央仓库 ~/.skills-panel/skills/)
                              ↓
                        Symlink/Copy → AI 工具目录
```

## 鸣谢

- **[skills-manager](https://github.com/xingkongliang/skills-manager)** — 参考了这个项目的很多有意思的设计和想法，包括数据库设计、安装流程、场景系统和项目工作区模块
- **[GitNexus](https://github.com/abhigyanpatwari/GitNexus)** — 代码分析工具，用于理解代码库结构和执行流程
- **[Understand-Anything](https://understand-anything.com/)** — 代码分析工具，用于生成交互式知识图谱
- **xiaomi mimo Orbit 百万亿 Token 计划** (https://platform.xiaomimimo.com/docs/zh-CN/news/v2.5-open-sourced) — 感谢提供的 Token 支持
- **商汤科技** — 感谢提供的免费 Token Plan，使用了 deepseek-v4-flash 模型进行开发

## License

MIT