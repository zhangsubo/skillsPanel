import { useState, useEffect } from 'react'
import { checkForUpdate } from '@/api/version'

const IGNORED_VERSION_KEY = 'skills-panel:ignored-update-version'

interface UpdateInfo {
  hasUpdate: boolean
  currentVersion: string
  latestVersion: string | null
}

export function useUpdateCheck() {
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null)
  const [checked, setChecked] = useState(false)
  const [dismissed, setDismissed] = useState(false)

  useEffect(() => {
    let cancelled = false

    async function runCheck() {
      try {
        const result = await checkForUpdate()
        if (!cancelled) {
          const ignored = localStorage.getItem(IGNORED_VERSION_KEY)
          if (result.latestVersion && ignored === result.latestVersion) {
            setDismissed(true)
          }
          setUpdateInfo(result)
          setChecked(true)
        }
      } catch {
        if (!cancelled) {
          setChecked(true)
        }
      }
    }

    runCheck()

    return () => {
      cancelled = true
    }
  }, [])

  const dismiss = (version: string) => {
    localStorage.setItem(IGNORED_VERSION_KEY, version)
    setDismissed(true)
  }

  const shouldShow = checked && updateInfo?.hasUpdate && !dismissed

  return { updateInfo, checked, shouldShow, dismiss }
}
