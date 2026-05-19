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

export default function UpdateDialog({
  open,
  onClose,
  currentVersion,
  latestVersion,
}: UpdateDialogProps) {
  const { t } = useTranslation()

  const handleConfirm = async () => {
    const url = 'https://github.com/zhangsubo/skillsPanel/releases'
    if (
      typeof window !== 'undefined' &&
      '__TAURI_INTERNALS__' in window
    ) {
      try {
        const { open: openShell } = await import('@tauri-apps/plugin-shell')
        await openShell(url)
      } catch {
        window.open(url, '_blank')
      }
    } else {
      window.open(url, '_blank')
    }
    onClose()
  }

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('updateDialog.title')}</DialogTitle>
          <DialogDescription>
            {t('updateDialog.description', {
              currentVersion,
              latestVersion,
            })}
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
