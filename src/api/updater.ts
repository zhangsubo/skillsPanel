import { check, type Update, type DownloadEvent } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'

export interface UpdateProgress {
  percent: number
}

export async function checkForUpdate(): Promise<Update | null> {
  try {
    const update = await check()
    return update
  } catch {
    return null
  }
}

export async function downloadAndInstallUpdate(
  onProgress?: (progress: UpdateProgress) => void,
): Promise<boolean> {
  try {
    const update = await check()
    if (!update) return false

    let totalBytes = 0
    let downloadedBytes = 0

    await update.downloadAndInstall((event: DownloadEvent) => {
      if (event.event === 'Started') {
        totalBytes = event.data.contentLength ?? 0
      } else if (event.event === 'Progress') {
        downloadedBytes += event.data.chunkLength
        const percent = totalBytes > 0 ? (downloadedBytes / totalBytes) * 100 : 0
        onProgress?.({ percent })
      }
    })

    await relaunch()
    return true
  } catch {
    return false
  }
}
