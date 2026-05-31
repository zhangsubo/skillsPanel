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

  const handleDownload = async () => {
    setDownloading(true)
    setProgress(0)

    const success = await downloadAndInstallUpdate((p) => {
      setProgress(Math.round(p.percent))
    })

    if (!success) {
      setDownloading(false)
      setProgress(0)
    }
  }

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('updateDialog.title')}</DialogTitle>
          <DialogDescription className="space-y-2">
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
