import { useState, useEffect, useCallback } from 'react'
import { getAppLogs, logMessage } from '@/api/logging'
import type { LogEntry } from '@/types'

export function useLogs() {
  const [logs, setLogs] = useState<LogEntry[]>([])
  const [loading, setLoading] = useState(false)

  const refresh = useCallback(async () => {
    setLoading(true)
    try {
      const data = await getAppLogs(200)
      setLogs(data)
    } catch (err) {
      console.error('Failed to fetch logs:', err)
    } finally {
      setLoading(false)
    }
  }, [])

  const sendLog = useCallback(async (level: string, message: string) => {
    try {
      await logMessage(level, message, 'frontend:manual')
      await refresh()
    } catch (err) {
      console.error('Failed to send log:', err)
    }
  }, [refresh])

  useEffect(() => {
    refresh()
    const id = setInterval(refresh, 3000)
    return () => clearInterval(id)
  }, [refresh])

  return { logs, loading, refresh, sendLog }
}
