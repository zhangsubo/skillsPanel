# Skills Panel CLI

独立的命令行工具，用于批量操作和自动化脚本场景。

## 安装

CLI 随 Skills Panel 一起构建：

```bash
cd src-tauri
cargo build --release --bin skills-cli
```

构建完成后，二进制文件位于 `target/release/skills-cli`。

## 快速开始

```bash
# 列出所有 skills
skills-cli list

# 列出已链接的 skills
skills-cli list --linked

# 以 JSON 格式输出（适合脚本处理）
skills-cli list --format json

# 安装本地 skill 并自动链接到 Cursor
skills-cli install ./my-skill --link cursor

# 批量链接所有 AI 组的 skills 到 Claude
skills-cli batch link --group "AI" --tools claude
```

## 命令参考

### list - 列出 skills

```bash
skills-cli list [OPTIONS]

Options:
  --tool <TOOL>      按工具名过滤
  --linked           只显示已链接的 skills
  --group <GROUP>    按组过滤
  --format <FORMAT>  输出格式：table（默认）、json、compact
```

**示例：**
```bash
# 列出所有 skills
skills-cli list

# 只显示链接到 Cursor 的 skills
skills-cli list --tool cursor

# 显示 AI 组的 skills
skills-cli list --group "library"

# 以紧凑格式输出（只有名称，适合管道）
skills-cli list --format compact | xargs -I {} skills-cli link {} --tool claude
```

### install - 安装 skill

```bash
skills-cli install <SOURCE> [OPTIONS]

Arguments:
  <SOURCE>           源路径（本地目录或 zip 文件）

Options:
  --name <NAME>      自定义 skill 名称
  --link <TOOLS>     自动链接到指定工具（逗号分隔）
  --force            强制覆盖已存在的 skill
```

**示例：**
```bash
# 安装本地目录
skills-cli install ./my-skill

# 安装并指定名称
skills-cli install ./my-skill --name custom-name

# 安装并自动链接到多个工具
skills-cli install ./my-skill --link cursor,claude,codex

# 强制覆盖
skills-cli install ./my-skill --force
```

### uninstall - 卸载 skill

```bash
skills-cli uninstall <NAME> [OPTIONS]

Arguments:
  <NAME>             Skill 名称

Options:
  --force, -f        跳过确认提示
```

**示例：**
```bash
# 卸载 skill
skills-cli uninstall my-skill

# 强制卸载（跳过确认）
skills-cli uninstall my-skill --force
```

### link - 链接 skill 到工具

```bash
skills-cli link <SKILL> [OPTIONS]

Arguments:
  <SKILL>            Skill 名称

Options:
  --tool <TOOL>      链接到单个工具
  --tools <TOOLS>    链接到多个工具（逗号分隔）
```

**示例：**
```bash
# 链接到单个工具
skills-cli link my-skill --tool cursor

# 链接到多个工具
skills-cli link my-skill --tools cursor,claude,codex
```

### unlink - 取消链接

```bash
skills-cli unlink <SKILL> [OPTIONS]

Arguments:
  <SKILL>            Skill 名称

Options:
  --tool <TOOL>      从指定工具取消链接
  --all              从所有工具取消链接
```

**示例：**
```bash
# 从单个工具取消链接
skills-cli unlink my-skill --tool cursor

# 从所有工具取消链接
skills-cli unlink my-skill --all
```

### scan - 扫描 skills

```bash
skills-cli scan [OPTIONS]

Options:
  --diff             显示差异
  --conflicts-only   只显示冲突
```

**示例：**
```bash
# 扫描所有源
skills-cli scan

# 显示差异
skills-cli scan --diff

# 只显示冲突
skills-cli scan --conflicts-only
```

### batch - 批量操作

#### batch link - 批量链接

```bash
skills-cli batch link [OPTIONS]

Options:
  --skills <SKILLS>  Skill 列表（逗号分隔）
  --file <FILE>      从文件读取 skill 列表（每行一个）
  --tools <TOOLS>    工具列表（逗号分隔）
  --pattern <PATTERN> 模式匹配（如 "ai-*"）
  --group <GROUP>    按组链接
```

**示例：**
```bash
# 批量链接指定 skills
skills-cli batch link --skills skill1,skill2,skill3 --tools cursor,claude

# 从文件读取 skill 列表
skills-cli batch link --file skills.txt --tool cursor

# 使用模式匹配
skills-cli batch link --pattern "ai-*" --tool claude

# 按组链接
skills-cli batch link --group "library" --tools cursor,claude,codex
```

#### batch delete - 批量删除

```bash
skills-cli batch delete [OPTIONS]

Options:
  --skills <SKILLS>  Skill 列表（逗号分隔）
  --force, -f        跳过确认
```

**示例：**
```bash
# 批量删除
skills-cli batch delete --skills skill1,skill2,skill3

# 强制删除
skills-cli batch delete --skills skill1,skill2 --force
```

#### batch export - 批量导出

```bash
skills-cli batch export [OPTIONS]

Options:
  --skills <SKILLS>  Skill 列表（逗号分隔）
  --linked           只导出已链接的 skills
  --output, -o <DIR> 输出目录
```

**示例：**
```bash
# 导出指定 skills
skills-cli batch export --skills skill1,skill2 --output ./backup

# 导出所有已链接的 skills
skills-cli batch export --linked --output ./backup
```

### config - 配置管理

#### config show - 显示配置

```bash
skills-cli config show
```

#### config set - 设置配置

```bash
skills-cli config set <KEY> <VALUE>

# 示例
skills-cli config set library_path ~/.my-skills
```

#### config add-tool - 添加工具

```bash
skills-cli config add-tool --name <NAME> --path <PATH>

# 示例
skills-cli config add-tool --name vscode --path ~/.vscode/skills
```

#### config add-source - 添加源

```bash
skills-cli config add-source --path <PATH> [--group <GROUP>]

# 示例
skills-cli config add-source --path ~/my-skills --group custom
```

#### config list-tools - 列出工具

```bash
skills-cli config list-tools
```

#### config enable/disable - 启用/禁用工具

```bash
skills-cli config enable <TOOL>
skills-cli config disable <TOOL>

# 示例
skills-cli config enable cursor
skills-cli config disable vscode
```

### tools - 管理工具

```bash
skills-cli tools [OPTIONS]

Options:
  --enabled          只显示启用的工具
```

**示例：**
```bash
# 列出所有工具
skills-cli tools

# 只显示启用的工具
skills-cli tools --enabled
```

### export - 导出 skill

```bash
skills-cli export <SKILL> --output <DIR>

# 示例
skills-cli export my-skill --output ./backup
```

### update - 更新 skill

```bash
skills-cli update <SKILL>

# 示例
skills-cli update my-skill
```

### tags - 标签管理

#### tags list - 列出标签

```bash
skills-cli tags list
```

#### tags create - 创建标签

```bash
skills-cli tags create --name <NAME> [--color <COLOR>]

# 示例
skills-cli tags create --name "Production" --color "#ff0000"
```

#### tags attach - 添加标签

```bash
skills-cli tags attach --skill <SKILL> --tag <TAG>

# 示例
skills-cli tags attach --skill my-skill --tag Production
```

#### tags detach - 移除标签

```bash
skills-cli tags detach --skill <SKILL> --tag <TAG>

# 示例
skills-cli tags detach --skill my-skill --tag Production
```

#### tags attach-batch - 批量添加标签

```bash
skills-cli tags attach-batch --skills <SKILLS> --tag <TAG>

# 示例
skills-cli tags attach-batch --skills skill1,skill2,skill3 --tag Production
```

## 输出格式

### 表格格式（默认）

```bash
skills-cli list
```

输出：
```
+------------------+---------+--------------+---------+
| Name             | Group   | Source       | Status  |
+------------------+---------+--------------+---------+
| my-skill         | library | local-folder | Linked  |
+------------------+---------+--------------+---------+
| other-skill      | library | git          | Unlinked|
+------------------+---------+--------------+---------+
```

### JSON 格式

```bash
skills-cli list --format json
```

输出：
```json
[
  {
    "name": "my-skill",
    "group": "library",
    "source": "local-folder",
    "tools": "cursor, claude",
    "status": "Linked"
  }
]
```

### 紧凑格式

```bash
skills-cli list --format compact
```

输出：
```
my-skill
other-skill
```

适合管道操作：
```bash
skills-cli list --format compact | xargs -I {} skills-cli link {} --tool cursor
```

## 常见场景

### 场景 1：批量安装和链接

```bash
# 从文件安装多个 skills
cat skills.txt | while read skill; do
  skills-cli install "$skill" --link cursor
done
```

### 场景 2：同步所有 skills 到新工具

```bash
# 链接所有已链接的 skills 到新工具
skills-cli list --linked --format compact | xargs -I {} skills-cli link {} --tool new-tool
```

### 场景 3：导出备份

```bash
# 导出所有已链接的 skills
skills-cli batch export --linked --output ./backup-$(date +%Y%m%d)
```

### 场景 4：按组管理

```bash
# 链接所有 library 组的 skills 到 Claude
skills-cli batch link --group "library" --tool claude

# 列出 library 组的 skills
skills-cli list --group "library"
```

### 场景 5：标签管理

```bash
# 创建标签
skills-cli tags create --name "Production" --color "#00ff00"

# 批量添加标签
skills-cli tags attach-batch --skills skill1,skill2,skill3 --tag Production

# 列出所有标签
skills-cli tags list
```

## 与 GUI 的关系

- **共享核心逻辑**：CLI 和 GUI 都使用 `core/` 模块
- **独立二进制**：CLI 是独立的可执行文件，不依赖 Tauri
- **共享数据库**：CLI 和 GUI 读写同一个 SQLite 数据库
- **互操作性**：CLI 的操作会立即反映到 GUI，反之亦然

## 错误处理

- 所有命令返回明确的退出码（0=成功，非0=失败）
- 错误信息输出到 stderr
- 支持 `--verbose` 模式输出详细日志
- 支持 `--quiet` 模式只输出错误

## 配置文件

CLI 使用与 GUI 相同的配置：
- 数据库：`~/.skills-panel/skills_panel.db`
- 中央仓库：`~/.skills-panel/skills/`
