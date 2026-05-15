import { useState, useEffect, useCallback } from 'react'
import { getConfig, updateConfig } from '@/api/config'
import { AppConfig } from '@/types'

export function useConfig() {
  const [config, setConfig] = useState<AppConfig | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<Error | null>(null)

  const refresh = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const data = await getConfig()
      setConfig(data)
    } catch (err) {
      setError(err instanceof Error ? err : new Error(String(err)))
    } finally {
      setLoading(false)
    }
  }, [])

  const update = useCallback(async (newConfig: AppConfig) => {
    setError(null)
    try {
      await updateConfig(newConfig)
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err : new Error(String(err)))
      throw err
    }
  }, [refresh])

  useEffect(() => {
    refresh()
  }, [refresh])

  return { config, loading, error, refresh, update }
}