import { check, type Update, type DownloadEvent } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'

export interface UpdateProgress {
  percent: number
}

export async function checkForUpdate(): Promise<Update | null> {
  try {
    const update = await check()
    return update
  } catch (e) {
    console.error('[updater] check() failed:', e)
    return null
  }
}

/**
 * Download, install, and relaunch the app with the latest update.
 * Returns `true` on success. On failure, throws with a user-readable
 * message so the caller can surface it.
 */
export async function downloadAndInstallUpdate(
  onProgress?: (progress: UpdateProgress) => void,
): Promise<boolean> {
  const update = await check()
  if (!update) {
    throw new Error(
      'Unable to locate a downloadable update. Ensure a signed ' +
        'release with a latest.json manifest is published and the ' +
        'Tauri updater pubkey is configured.',
    )
  }

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
}
