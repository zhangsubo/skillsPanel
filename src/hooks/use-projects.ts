// 薄壳：useProjects 保持原签名，state 由 ProjectsProvider 共享。
// 根因 2 修复：所有消费 useProjects 的组件（Sidebar / AddProjectDialog / ProjectWorkspace）
// 看到同一份 state，而不是各 useState 独立。详见 projects-context.tsx。
import { useProjectsContext } from './projects-context'

export function useProjects() {
  return useProjectsContext()
}
