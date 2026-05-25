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

interface UpdateDialogProps {
  open: boolean
  onClose: () => void
  currentVersion: string
  latestVersion: string
}

const RELEASES_BASE_URL = 'https://github.com/zhangsubo/skillsPanel/releases'

export default function UpdateDialog({
  open,
  onClose,
  currentVersion,
  latestVersion,
}: UpdateDialogProps) {
  const { t } = useTranslation()

  const downloadUrl = `${RELEASES_BASE_URL}/tag/${latestVersion}`

  const handleConfirm = async () => {
    if (
      typeof window !== 'undefined' &&
      '__TAURI_INTERNALS__' in window
    ) {
      try {
        const { open: openShell } = await import('@tauri-apps/plugin-shell')
        await openShell(downloadUrl)
      } catch {
        window.open(downloadUrl, '_blank')
      }
    } else {
      window.open(downloadUrl, '_blank')
    }
    onClose()
  }

  const handleOpenUrl = async () => {
    if (
      typeof window !== 'undefined' &&
      '__TAURI_INTERNALS__' in window
    ) {
      try {
        const { open: openShell } = await import('@tauri-apps/plugin-shell')
        await openShell(downloadUrl)
      } catch {
        window.open(downloadUrl, '_blank')
      }
    } else {
      window.open(downloadUrl, '_blank')
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
            <button
              type="button"
              onClick={handleOpenUrl}
              className="break-all text-left text-xs text-primary underline hover:text-primary/80"
              title={downloadUrl}
            >
              {downloadUrl}
            </button>
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            {t('updateDialog.cancel')}
          </Button>
          <Button onClick={handleConfirm}>
            {t('updateDialog.confirm')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
