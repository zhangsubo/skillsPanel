import { check, type Update, type DownloadEvent } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import { logMessage } from '@/api/logging'

export interface UpdateProgress {
  percent: number
}

const LOG_SOURCE = 'updater'

function describeError(e: unknown): string {
  if (e instanceof Error) {
    return `${e.name}: ${e.message}${e.stack ? `\n${e.stack}` : ''}`
  }
  let json = ''
  try {
    json = JSON.stringify(e)
  } catch {
    json = '<unserializable>'
  }
  return `typeof=${typeof e} string=${String(e)} json=${json}`
}

function log(level: string, message: string): void {
  // 不 await，避免日志写入阻塞更新主流程；日志失败也不影响更新
  void logMessage(level, message, LOG_SOURCE).catch(() => {})
}

export async function checkForUpdate(): Promise<Update | null> {
  try {
    const update = await check()
    return update
  } catch (e) {
    console.error('[updater] check() failed:', e)
    log('error', `check() failed: ${describeError(e)}`)
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
  log('info', 'update flow start, calling check()')
  const update = await check()
  log(
    'info',
    update
      ? `check() ok: current=${update.currentVersion} available=${update.version}`
      : 'check() returned null (no update)',
  )
  if (!update) {
    throw new Error(
      'Unable to locate a downloadable update. Ensure a signed ' +
        'release with a latest.json manifest is published and the ' +
        'Tauri updater pubkey is configured.',
    )
  }

  let totalBytes = 0
  let downloadedBytes = 0

  try {
    await update.downloadAndInstall((event: DownloadEvent) => {
      if (event.event === 'Started') {
        totalBytes = event.data.contentLength ?? 0
        log('info', `download started, contentLength=${totalBytes}`)
      } else if (event.event === 'Progress') {
        downloadedBytes += event.data.chunkLength
        const percent = totalBytes > 0 ? (downloadedBytes / totalBytes) * 100 : 0
        onProgress?.({ percent })
      } else if (event.event === 'Finished') {
        log('info', 'download finished, verifying signature and installing')
      }
    })
    log('info', 'downloadAndInstall resolved, install ok')
  } catch (e) {
    log('error', `downloadAndInstall failed: ${describeError(e)}`)
    throw e
  }

  try {
    log('info', 'install ok, calling relaunch()')
    await relaunch()
    log('info', 'relaunch() resolved')
  } catch (e) {
    log('error', `relaunch failed: ${describeError(e)}`)
    throw e
  }
  return true
}
