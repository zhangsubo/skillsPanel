import { useState, useEffect, useCallback, useRef } from 'react'
import { getLibrary, scanSkills, getSkillContent, getTools, linkSkill, unlinkSkill, deleteSkill } from '@/api/library'
import { getInstalledSkillsFromDb } from '@/api/database'
import { ScanResult, Skill, Tool } from '@/types'

export function useLibrary() {
  const [skillNames, setSkillNames] = useState<string[]>([])
  const [installedSkills, setInstalledSkills] = useState<Skill[]>([])
  const [scanResult, setScanResult] = useState<ScanResult | null>(null)
  const [tools, setTools] = useState<Tool[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<Error | null>(null)
  const refreshRequestId = useRef(0)
  const pendingRefreshes = useRef(0)

  const refresh = useCallback(async () => {
    const requestId = ++refreshRequestId.current
    pendingRefreshes.current += 1
    setLoading(true)
    setError(null)
    try {
      try {
        const dbSkills = await getInstalledSkillsFromDb()
        if (requestId === refreshRequestId.current) {
          setInstalledSkills(dbSkills)
          setSkillNames(dbSkills.map((skill) => skill.name))
        }
      } catch {
        const names = await getLibrary()
        if (requestId === refreshRequestId.current) {
          setInstalledSkills([])
          setSkillNames(names)
        }
      }
    } catch (err) {
      if (requestId === refreshRequestId.current) {
        setError(err instanceof Error ? err : new Error(String(err)))
      }
    } finally {
      pendingRefreshes.current -= 1
      if (pendingRefreshes.current === 0) {
        setLoading(false)
      }
    }
  }, [])

  const scan = useCallback(async () => {
    setError(null)
    try {
      const result = await scanSkills()
      setScanResult(result)
      return result
    } catch (err) {
      setError(err instanceof Error ? err : new Error(String(err)))
      throw err
    }
  }, [])

  const getContent = useCallback(async (skillId: string) => {
    try {
      return await getSkillContent(skillId)
    } catch (err) {
      setError(err instanceof Error ? err : new Error(String(err)))
      throw err
    }
  }, [])

  const fetchTools = useCallback(async () => {
    try {
      const result = await getTools()
      setTools(result)
      return result
    } catch (err) {
      setError(err instanceof Error ? err : new Error(String(err)))
      throw err
    }
  }, [])

  const handleLinkSkill = useCallback(async (skillName: string, toolId: string) => {
    await linkSkill(skillName, toolId)
  }, [])

  const handleUnlinkSkill = useCallback(async (skillName: string, toolId: string) => {
    await unlinkSkill(skillName, toolId)
  }, [])

  const handleDeleteSkill = useCallback(async (skillName: string) => {
    await deleteSkill(skillName)
  }, [])

  useEffect(() => {
    refresh()
  }, [refresh])

  return {
    skillNames,
    installedSkills,
    scanResult,
    tools,
    loading,
    error,
    refresh,
    scan,
    getContent,
    fetchTools,
    linkSkill: handleLinkSkill,
    unlinkSkill: handleUnlinkSkill,
    deleteSkill: handleDeleteSkill,
  }
}
