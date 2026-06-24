import { useState, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import { createTag, bulkAttachTag, listTags } from '@/api/tags'
import type { Tag } from '@/types'

type TagMode = 'suggested' | 'existing' | 'new'

interface BatchTagDialogProps {
  open: boolean
  skillIds: string[]
  suggestedName: string
  onClose: () => void
  onComplete: () => void
}

export default function BatchTagDialog({ open, skillIds, suggestedName, onClose, onComplete }: BatchTagDialogProps) {
  const { t } = useTranslation()
  const [mode, setMode] = useState<TagMode>('suggested')
  const [existingTags, setExistingTags] = useState<Tag[]>([])
  const [selectedTagId, setSelectedTagId] = useState<string>('')
  const [newTagName, setNewTagName] = useState(suggestedName)
  const [loading, setLoading] = useState(false)

  useEffect(() => {
    if (open) {
      listTags().then(setExistingTags).catch(() => {})
      setNewTagName(suggestedName)
      setMode('suggested')
      setSelectedTagId('')
    }
  }, [open, suggestedName])

  const handleConfirm = async () => {
    setLoading(true)
    try {
      let tagId = selectedTagId
      if (mode === 'suggested' || mode === 'new') {
        const name = mode === 'suggested' ? suggestedName : newTagName.trim()
        if (!name) return
        const existing = existingTags.find((t) => t.name === name)
        if (existing) {
          tagId = existing.id
        } else {
          const tag = await createTag(name, undefined, undefined)
          tagId = tag.id
        }
      }
      if (!tagId) return
      await bulkAttachTag(skillIds, tagId)
      onComplete()
    } catch (e) {
      console.error('Batch tag failed:', e)
    } finally {
      setLoading(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>{t('batchTag.title')}</DialogTitle>
        </DialogHeader>

        <div className="space-y-3 text-sm">
          <p className="text-muted-foreground">
            {t('batchTag.description', { count: skillIds.length })}
          </p>

          {/* Mode tabs */}
          <div className="flex gap-1 rounded-md bg-muted p-1">
            {(['suggested', 'existing', 'new'] as TagMode[]).map((m) => (
              <button
                key={m}
                onClick={() => setMode(m)}
                className={[
                  'flex-1 rounded px-2 py-1 text-xs font-medium transition-colors',
                  mode === m
                    ? 'bg-background text-foreground shadow-sm'
                    : 'text-muted-foreground hover:text-foreground',
                ].join(' ')}
              >
                {t(`batchTag.mode.${m}`)}
              </button>
            ))}
          </div>

          {/* Suggested */}
          {mode === 'suggested' && (
            <div className="flex items-center gap-2">
              <Badge variant="secondary">{suggestedName}</Badge>
              <span className="text-muted-foreground text-xs">
                ({skillIds.length} {t('batchTag.skills')})
              </span>
            </div>
          )}

          {/* Existing */}
          {mode === 'existing' && (
            <div className="max-h-40 space-y-1 overflow-y-auto">
              {existingTags.length === 0 ? (
                <p className="text-muted-foreground text-xs">{t('batchTag.noTags')}</p>
              ) : (
                existingTags.map((tag) => (
                  <button
                    key={tag.id}
                    onClick={() => setSelectedTagId(tag.id)}
                    className={[
                      'flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-sm',
                      selectedTagId === tag.id
                        ? 'bg-primary/10 text-primary'
                        : 'hover:bg-muted',
                    ].join(' ')}
                  >
                    {tag.color && (
                      <span className="h-2 w-2 rounded-full" style={{ backgroundColor: tag.color }} />
                    )}
                    {tag.name}
                  </button>
                ))
              )}
            </div>
          )}

          {/* New */}
          {mode === 'new' && (
            <Input
              value={newTagName}
              onChange={(e) => setNewTagName(e.target.value)}
              placeholder={t('batchTag.newPlaceholder')}
            />
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={onClose}>{t('batchTag.skip')}</Button>
          <Button onClick={handleConfirm} disabled={loading || (mode === 'existing' && !selectedTagId) || ((mode === 'new') && !newTagName.trim())}>
            {loading ? t('batchTag.tagging') : t('batchTag.confirm')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
