import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from 'react'
import { listProjects, scanProject, createProject, deleteProject } from '@/api/projects'
import type { Project, ProjectDto } from '@/types'

export type ProjectsContextValue = {
  projects: Project[]
  selectedProjectId: string | null
  projectDetail: ProjectDto | null
  loading: boolean
  scanning: boolean
  error: Error | null
  refresh: () => Promise<void>
  selectProject: (projectId: string) => Promise<void>
  /** 返回新创建项目的 id（根因 3 前端契约）。 */
  addProject: (name: string, rootPath: string) => Promise<string>
  removeProject: (projectId: string) => Promise<void>
}

const ProjectsContext = createContext<ProjectsContextValue | null>(null)

/**
 * 共享 projects state 的 Provider。
 * 根因 2 修复：替代原 useProjects 的 useState 写死，让 Sidebar / AddProjectDialog /
 * ProjectWorkspace 三个组件看到同一份 state（修复"添加后半天才显示"）。
 */
export function ProjectsProvider({ children }: { children: ReactNode }) {
  const [projects, setProjects] = useState<Project[]>([])
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null)
  const [projectDetail, setProjectDetail] = useState<ProjectDto | null>(null)
  const [loading, setLoading] = useState(true)
  const [scanning, setScanning] = useState(false)
  const [error, setError] = useState<Error | null>(null)

  const refresh = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const list = await listProjects()
      setProjects(list)
      setSelectedProjectId((current) => {
        if (current && !list.find((p) => p.id === current)) {
          setProjectDetail(null)
          return null
        }
        return current
      })
    } catch (err) {
      setError(err instanceof Error ? err : new Error(String(err)))
    } finally {
      setLoading(false)
    }
  }, [])

  const selectProject = useCallback(async (projectId: string) => {
    setSelectedProjectId(projectId)
    setScanning(true)
    setError(null)
    try {
      const detail = await scanProject(projectId)
      setProjectDetail(detail)
    } catch (err) {
      setError(err instanceof Error ? err : new Error(String(err)))
    } finally {
      setScanning(false)
    }
  }, [])

  const addProject = useCallback(
    async (name: string, rootPath: string): Promise<string> => {
      const id = await createProject(name, rootPath)
      // 共享 state：refresh 后所有消费组件都会看到新条目（根因 2 修复）
      await refresh()
      return id
    },
    [refresh],
  )

  const removeProject = useCallback(
    async (projectId: string) => {
      await deleteProject(projectId)
      setSelectedProjectId((current) => {
        if (current === projectId) {
          setProjectDetail(null)
          return null
        }
        return current
      })
      await refresh()
    },
    [refresh],
  )

  // 首次挂载触发 refresh
  useEffect(() => {
    refresh()
  }, [refresh])

  const value: ProjectsContextValue = {
    projects,
    selectedProjectId,
    projectDetail,
    loading,
    scanning,
    error,
    refresh,
    selectProject,
    addProject,
    removeProject,
  }

  return <ProjectsContext.Provider value={value}>{children}</ProjectsContext.Provider>
}

export function useProjectsContext(): ProjectsContextValue {
  const ctx = useContext(ProjectsContext)
  if (!ctx) {
    throw new Error('useProjectsContext must be used inside <ProjectsProvider>')
  }
  return ctx
}
