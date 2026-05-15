import { useState, useEffect, useCallback } from 'react'
import { getTools } from '@/api/tools'
import { linkSkill, unlinkSkill } from '@/api/linking'
import { syncSkills } from '@/api/sync'
import { Tool } from '@/types'

export function useTools() {
  const [tools, setTools] = useState<Tool[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<Error | null>(null)

  const refresh = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const data = await getTools()
      setTools(data)
    } catch (err) {
      setError(err instanceof Error ? err : new Error(String(err)))
    } finally {
      setLoading(false)
    }
  }, [])

  const link = useCallback(async (name: string, toolId: string) => {
    setError(null)
    try {
      await linkSkill(name, toolId)
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err : new Error(String(err)))
      throw err
    }
  }, [refresh])

  const unlink = useCallback(async (name: string, toolId: string) => {
    setError(null)
    try {
      await unlinkSkill(name, toolId)
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err : new Error(String(err)))
      throw err
    }
  }, [refresh])

  const sync = useCallback(async () => {
    setError(null)
    try {
      await syncSkills()
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err : new Error(String(err)))
      throw err
    }
  }, [refresh])

  useEffect(() => {
    refresh()
  }, [refresh])

  return { tools, loading, error, refresh, link, unlink, sync }
}