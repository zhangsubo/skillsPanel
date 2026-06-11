import { useState } from 'react'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { useTranslation } from 'react-i18next'
import { downloadAndInstallUpdate } from '@/api/updater'

interface UpdateDialogProps {
  open: boolean
  onClose: () => void
  currentVersion: string
  latestVersion: string
}

export default function UpdateDialog({
  open,
  onClose,
  currentVersion,
  latestVersion,
}: UpdateDialogProps) {
  const { t } = useTranslation()
  const [downloading, setDownloading] = useState(false)
  const [progress, setProgress] = useState(0)
  const [error, setError] = useState<string | null>(null)

  const handleDownload = async () => {
    setDownloading(true)
    setProgress(0)
    setError(null)

    try {
      await downloadAndInstallUpdate((p) => {
        setProgress(Math.round(p.percent))
      })
    } catch (e) {
      setDownloading(false)
      setProgress(0)
      // Surface the failure to the user so they know to retry or
      // check their network / Tauri capabilities.
      setError(e instanceof Error ? e.message : t('updateDialog.error'))
    }
  }

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('updateDialog.title')}</DialogTitle>
          <DialogDescription className="space-y-2">
            {error && (
              <div className="rounded-lg bg-red-50 px-4 py-3 text-sm text-red-600">
                {error}
              </div>
            )}
            <p>
              {t('updateDialog.description', {
                currentVersion,
                latestVersion,
              })}
            </p>
            {downloading && (
              <div className="space-y-2">
                <div className="h-2 w-full overflow-hidden rounded-full bg-secondary">
                  <div
                    className="h-full bg-primary transition-all duration-300"
                    style={{ width: `${progress}%` }}
                  />
                </div>
                <p className="text-xs text-muted-foreground">
                  {t('updateDialog.downloading', { progress })}
                </p>
              </div>
            )}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" onClick={onClose} disabled={downloading}>
            {t('updateDialog.cancel')}
          </Button>
          <Button onClick={handleDownload} disabled={downloading}>
            {downloading
              ? t('updateDialog.installing')
              : t('updateDialog.updateNow')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
