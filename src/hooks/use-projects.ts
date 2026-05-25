import { useState, useEffect, useCallback } from 'react'
import { listProjects, scanProject, createProject, deleteProject } from '@/api/projects'
import type { Project, ProjectDto } from '@/types'

export function useProjects() {
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
      if (selectedProjectId && !list.find((p) => p.id === selectedProjectId)) {
        setSelectedProjectId(null)
        setProjectDetail(null)
      }
    } catch (err) {
      setError(err instanceof Error ? err : new Error(String(err)))
    } finally {
      setLoading(false)
    }
  }, [selectedProjectId])

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

  const addProject = useCallback(async (name: string, rootPath: string) => {
    await createProject(name, rootPath)
    await refresh()
  }, [refresh])

  const removeProject = useCallback(async (projectId: string) => {
    await deleteProject(projectId)
    if (selectedProjectId === projectId) {
      setSelectedProjectId(null)
      setProjectDetail(null)
    }
    await refresh()
  }, [selectedProjectId, refresh])

  useEffect(() => {
    refresh()
  }, [refresh])

  return {
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
}
