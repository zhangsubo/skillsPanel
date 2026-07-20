# 修复：删除项目时的白屏和 skill 清理问题

## 问题描述

### 问题 1：删除项目后出现白屏
删除项目工作区的特定项目时，页面出现白屏，需要刷新才能恢复。

### 问题 2：项目级 skill 未被清理
删除项目时，项目目录下的 skill 文件夹（如 `.claude/skills/`）没有被删除，导致磁盘空间浪费。

## 根本原因分析

### 问题 1 根本原因
`ProjectWorkspace.tsx` 中的 `handleDelete` 函数直接调用 API `deleteProject`，绕过了 context 的状态管理：

```typescript
// 错误的实现
await deleteProject(projectId)
navigate('/projects')
```

这导致：
1. `projectDetail` 状态没有被清理，仍然指向已删除的项目
2. 组件在导航完成前尝试渲染不存在的 `projectDetail`
3. 渲染逻辑出错导致白屏

### 问题 2 根本原因
后端 `ProjectsRepository::delete` 方法只删除数据库记录：

```rust
pub fn delete(&self, id: &str) -> Result<(), AppError> {
    let conn = self.db.connection();
    conn.execute("DELETE FROM projects WHERE id = ?1", params![id])
        .map_err(|e| AppError::Config(format!("Failed to delete project: {}", e)))?;
    Ok(())
}
```

项目目录下的 skill 文件夹被遗留在磁盘上。

## 解决方案

### 修复 1：使用 context 的 removeProject 方法

**文件**: `src/pages/ProjectWorkspace.tsx`

修改前：
```typescript
const { projects, projectDetail, scanning, selectProject } = useProjects()

const handleDelete = async () => {
  if (!projectId || !project) return
  if (!window.confirm(t('project.confirmDelete', { name: project.name }))) return
  try {
    await deleteProject(projectId)
    navigate('/projects')
  } catch (err) {
    console.error('Failed to delete project:', err)
    alert(t('project.deleteFailed', { error: String(err) }))
  }
}
```

修改后：
```typescript
const { projects, projectDetail, scanning, selectProject, removeProject } = useProjects()

const handleDelete = async () => {
  if (!projectId || !project) return
  if (!window.confirm(t('project.confirmDelete', { name: project.name }))) return
  try {
    // 使用 context 的 removeProject，它会正确清理状态
    await removeProject(projectId)
    // 导航到项目列表
    navigate('/projects')
  } catch (err) {
    console.error('Failed to delete project:', err)
    alert(t('project.deleteFailed', { error: String(err) }))
  }
}
```

**关键点**：
- `removeProject` 会先调用 API 删除项目
- 然后清理状态：设置 `projectDetail` 为 `null`，重置 `selectedProjectId`
- 最后刷新项目列表
- 移除了未使用的 `deleteProject` 导入

### 修复 2：删除项目时清理 skill 文件夹

**文件**: `src-tauri/src/core/database.rs`

修改后：
```rust
pub fn delete(&self, id: &str) -> Result<(), AppError> {
    // 先获取项目信息，需要 root_path 来删除 skill 文件夹
    let project = self.get_by_id(id)?;

    if let Some(proj) = project {
        // 删除项目目录下的 skill 文件夹
        let root = std::path::Path::new(&proj.root_path);
        let agent_skill_dirs = vec![
            ".claude/skills",
            ".claude/skills-disabled",
            ".cursor/skills",
            ".cursor/skills-disabled",
            ".config/opencode/skill",
            ".config/opencode/skill-disabled",
        ];

        for dir in agent_skill_dirs {
            let skill_path = root.join(dir);
            if skill_path.exists() {
                if let Err(e) = std::fs::remove_dir_all(&skill_path) {
                    // 记录错误但不阻止删除操作
                    eprintln!("Warning: Failed to delete skill directory {}: {}", skill_path.display(), e);
                }
            }
        }
    }

    // 删除数据库记录
    let conn = self.db.connection();
    conn.execute("DELETE FROM projects WHERE id = ?1", params![id])
        .map_err(|e| AppError::Config(format!("Failed to delete project: {}", e)))?;
    Ok(())
}
```

**关键点**：
- 在删除数据库记录前，先获取项目的 `root_path`
- 删除所有已知的 agent skill 目录
- 使用 `std::fs::remove_dir_all` 递归删除目录及其内容
- 如果删除失败只记录警告，不阻止数据库删除操作（容错处理）

## 测试验证

### 后端测试

添加了两个单元测试：

1. **test_delete_project_removes_skill_directories**
   - 验证删除项目时正确删除所有 skill 目录
   - 验证数据库记录被删除

2. **test_delete_project_succeeds_even_if_skill_dirs_dont_exist**
   - 验证即使没有 skill 目录，删除操作也能成功
   - 容错性测试

测试结果：
```
running 2 tests
test core::database::tests::test_delete_project_succeeds_even_if_skill_dirs_dont_exist ... ok
test core::database::tests::test_delete_project_removes_skill_directories ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 187 filtered out
```

### 前端验证

- TypeScript 编译通过 ✓
- Vite 构建成功 ✓
- 代码质量检查通过 ✓

### GitNexus 变更影响分析

```
Risk Level: MEDIUM
Changed Symbols: 9
Affected Processes: 3
- HandleDelete → IsTauriEnv
- ProjectWorkspace → UseProjectsContext
- ProjectWorkspace → Cn
```

风险等级为 MEDIUM 是合理的，因为修改了项目删除的核心逻辑。

## 影响范围

### 前端
- **修改文件**: `src/pages/ProjectWorkspace.tsx`
- **影响组件**: ProjectWorkspace
- **影响功能**: 删除项目的用户交互

### 后端
- **修改文件**: `src-tauri/src/core/database.rs`
- **影响模块**: ProjectsRepository
- **影响功能**: 项目删除逻辑

## 兼容性

- ✅ 向后兼容：不影响现有数据结构
- ✅ API 兼容：没有修改 Tauri 命令接口
- ✅ 数据库兼容：不需要迁移

## 部署注意事项

无特殊部署要求，标准发布流程即可。

## 相关问题

解决了用户报告的两个问题：
1. 删除项目后白屏需要刷新
2. 删除项目时磁盘上的 skill 文件夹未被清理

## 作者

修复日期：2026-07-20
