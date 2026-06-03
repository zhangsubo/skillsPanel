import { useState, useEffect, useCallback, useMemo } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { useLibrary } from '@/hooks/use-library'
import { useTags } from '@/hooks/use-tags'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog'
import { TagChip } from '@/components/TagChip'
import { TagManagerDialog } from '@/components/TagManagerDialog'
import { ArrowLeft, Folder, Loader2, Plus, Tag as TagIcon } from 'lucide-react'
import type { Tag } from '@/types'

const safeErrorMessage = (e: unknown): string =>
  e instanceof Error ? e.message : String(e);

export default function SkillDetail() {
  const { skillName } = useParams<{ skillName: string }>()
  const { t } = useTranslation()
  const navigate = useNavigate()
  const { installedSkills, scanResult, scan, tools, fetchTools, getContent, linkSkill, unlinkSkill } = useLibrary()
  const { tags: allTags, loading: tagsLoading, refresh: refreshTags, attach, detach, tagsForSkill } = useTags()

  const [content, setContent] = useState<string | null>(null)
  const [contentLoading, setContentLoading] = useState(true)
  const [contentError, setContentError] = useState<Error | null>(null)
  const [togglingTool, setTogglingTool] = useState<string | null>(null)
  const [localLinkedTools, setLocalLinkedTools] = useState<Set<string>>(new Set())
  const [toggleError, setToggleError] = useState<string | null>(null)

  // Tag UI state — kept local to this page so the global tag list cache isn't
  // disturbed by per-skill attach/detach optimistic updates.
  const [attachedTags, setAttachedTags] = useState<Tag[]>([])
  const [tagsBusy, setTagsBusy] = useState(false)
  const [addTagOpen, setAddTagOpen] = useState(false)
  const [manageOpen, setManageOpen] = useState(false)
  const [tagError, setTagError] = useState<string | null>(null)

  const decodedName = useMemo(() => {
    if (!skillName) return ''
    try {
      return decodeURIComponent(skillName)
    } catch {
      return ''
    }
  }, [skillName])

  useEffect(() => {
    if (!scanResult) {
      scan()
    }
  }, [scanResult, scan])

  useEffect(() => {
    fetchTools()
  }, [fetchTools])

  const scannedSkill = scanResult?.skills.find(
    (s) => s.skill.name === decodedName,
  )

  const installedSkill = installedSkills.find(
    (s) => s.name === decodedName,
  )

  const skillWithStatus = useMemo(() => {
    if (scannedSkill) return scannedSkill
    if (installedSkill) {
      return {
        skill: {
          ...installedSkill,
          description: installedSkill.description || 'No description',
        },
        tool_statuses: {},
        rule_decisions: {},
      }
    }
    return undefined
  }, [scannedSkill, installedSkill])

  const loadContent = useCallback(async () => {
    if (!skillWithStatus) return
    setContentLoading(true)
    setContentError(null)
    try {
      const md = await getContent(skillWithStatus.skill.name)
      setContent(md)
    } catch (err) {
      setContentError(err instanceof Error ? err : new Error(String(err)))
    } finally {
      setContentLoading(false)
    }
  }, [skillWithStatus, getContent])

  useEffect(() => {
    loadContent()
  }, [loadContent])

  const scannedLinkedTools = skillWithStatus
    ? Object.entries(skillWithStatus.tool_statuses)
        .filter(([, status]) => status === 'linked')
        .map(([toolId]) => toolId)
    : []

  const linkedToolIds = [...new Set([...scannedLinkedTools, ...localLinkedTools])]

  const enabledTools = tools.filter((t) => t.enabled)

  const handleToggle = async (toolId: string, isLinked: boolean) => {
    setTogglingTool(toolId)
    setToggleError(null)
    try {
      if (isLinked) {
        await unlinkSkill(decodedName, toolId)
        setLocalLinkedTools((prev) => {
          const next = new Set(prev)
          next.delete(toolId)
          return next
        })
      } else {
        await linkSkill(decodedName, toolId)
        setLocalLinkedTools((prev) => new Set(prev).add(toolId))
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      setToggleError(message)
    } finally {
      setTogglingTool(null)
    }
  }

  const syncedCount = enabledTools.filter((tool) =>
    linkedToolIds.includes(tool.id),
  ).length

  // ── Tag fetch + handlers ────────────────────────────────────────
  // skillId is the stable DB row id; tag attach/detach is keyed on it,
  // not on the human-readable skill name.
  const skillId = skillWithStatus?.skill.id

  const fetchAttachedTags = useCallback(async () => {
    if (!skillId) {
      setAttachedTags([])
      return
    }
    try {
      const list = await tagsForSkill(skillId)
      setAttachedTags(list)
    } catch (err) {
      setTagError(safeErrorMessage(err))
    }
  }, [skillId, tagsForSkill])

  // Refetch attached tags whenever the page identifies a new skill.
  useEffect(() => {
    void fetchAttachedTags()
  }, [fetchAttachedTags])

  // Take the full Tag object so the optimistic insert is independent of the
  // global tag list cache (which may not be loaded yet, or may be stale after
  // the global TagManagerDialog creates new tags from another component).
  const handleAttach = useCallback(
    async (tag: Tag) => {
      if (!skillId) return
      setTagsBusy(true)
      setTagError(null)
      try {
        await attach(skillId, tag.id)
        setAttachedTags((prev) =>
          prev.some((t) => t.id === tag.id) ? prev : [...prev, tag],
        )
      } catch (err) {
        setTagError(safeErrorMessage(err))
      } finally {
        setTagsBusy(false)
      }
    },
    [skillId, attach],
  )

  const handleDetach = useCallback(
    async (tagId: string) => {
      if (!skillId) return
      setTagsBusy(true)
      setTagError(null)
      try {
        await detach(skillId, tagId)
        setAttachedTags((prev) => prev.filter((t) => t.id !== tagId))
      } catch (err) {
        setTagError(safeErrorMessage(err))
      } finally {
        setTagsBusy(false)
      }
    },
    [skillId, detach],
  )

  // Refetch after the global manager closes — tag list or attach set may have changed.
  const handleManageClose = useCallback(() => {
    setManageOpen(false)
    void refreshTags()
    void fetchAttachedTags()
  }, [refreshTags, fetchAttachedTags])

  const attachedTagIds = useMemo(
    () => new Set(attachedTags.map((t) => t.id)),
    [attachedTags],
  )
  const availableTags = useMemo(
    () => allTags.filter((tg) => !attachedTagIds.has(tg.id)),
    [allTags, attachedTagIds],
  )

  if (!scanResult) {
    return (
      <div className="flex h-full items-center justify-center p-6">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    )
  }

  if (!skillWithStatus) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4 p-6">
        <p className="text-muted-foreground">{t('library.skillNotFound')}</p>
        <Button variant="outline" onClick={() => navigate('/library')}>
          <ArrowLeft className="mr-2 h-4 w-4" />
          {t('library.back')}
        </Button>
      </div>
    )
  }

  const { skill } = skillWithStatus

  return (
    <div className="flex h-full flex-col gap-6 p-6 overflow-auto">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <Button
            variant="ghost"
            size="icon"
            className="h-8 w-8"
            onClick={() => navigate('/library')}
          >
            <ArrowLeft className="h-4 w-4" />
          </Button>
          <h2 className="text-2xl font-semibold text-foreground">{skill.name}</h2>
        </div>
        <div className="flex items-center gap-1.5">
          {linkedToolIds.map((toolId) => (
            <span
              key={toolId}
              className="inline-flex h-7 items-center rounded-full bg-primary/10 px-2.5 text-xs font-medium text-primary dark:bg-primary/20"
            >
              {toolId.replace(/[-_]/g, ' ').split(' ').map(w => w[0]?.toUpperCase()).join('').slice(0, 2)}
            </span>
          ))}
        </div>
      </div>

      <Card>
        <CardContent className="flex items-center gap-3 p-4">
          <Folder className="h-4 w-4 shrink-0 text-muted-foreground" />
          <span className="truncate text-sm text-muted-foreground" title={skill.library_path}>
            {skill.library_path}
          </span>
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="pb-3">
          <div className="flex items-center justify-between">
            <CardTitle className="text-base">{t('library.agents')}</CardTitle>
            <span className="text-sm text-muted-foreground">
              {t('library.syncStatus')}: {syncedCount} / {enabledTools.length}
            </span>
          </div>
        </CardHeader>
        <CardContent>
          {toggleError && (
            <div className="mb-3 rounded-lg bg-red-50 px-4 py-3 text-sm text-red-600">
              {toggleError}
            </div>
          )}
          {enabledTools.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t('library.noAgents')}</p>
          ) : (
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
              {enabledTools.map((tool) => {
                const isLinked = linkedToolIds.includes(tool.id)
                const isToggling = togglingTool === tool.id

                return (
                  <div
                    key={tool.id}
                    className="flex items-center justify-between rounded-lg border p-3 transition-colors hover:bg-muted/50"
                  >
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-sm font-medium">{tool.name}</p>
                      <p className="truncate text-xs text-muted-foreground">{tool.path}</p>
                    </div>
                    <button
                      type="button"
                      role="switch"
                      aria-checked={isLinked}
                      disabled={isToggling}
                      onClick={() => handleToggle(tool.id, isLinked)}
                      className={`relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 ${
                        isLinked ? 'bg-primary' : 'bg-muted'
                      }`}
                    >
                      <span
                        className={`pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out ${
                          isLinked ? 'translate-x-5' : 'translate-x-0'
                        }`}
                      />
                    </button>
                  </div>
                )
              })}
            </div>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="pb-3">
          <div className="flex items-center justify-between">
            <CardTitle className="flex items-center gap-2 text-base">
              <TagIcon className="h-4 w-4" />
              {t('detail.tags')}
              {attachedTags.length > 0 && (
                <span className="text-xs font-normal text-muted-foreground">
                  {t('detail.tagsCount', { count: attachedTags.length })}
                </span>
              )}
            </CardTitle>
            <div className="flex items-center gap-1.5">
              <Button
                variant="outline"
                size="sm"
                className="h-7 text-xs"
                onClick={() => setAddTagOpen(true)}
                disabled={tagsLoading}
                data-testid="add-tag-btn"
              >
                <Plus className="mr-1 h-3.5 w-3.5" />
                {t('detail.addTag')}
              </Button>
              <Button
                variant="ghost"
                size="sm"
                className="h-7 text-xs"
                onClick={() => setManageOpen(true)}
              >
                {t('detail.manageTags')}
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          {tagError && (
            <div className="mb-3 rounded-lg border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
              {tagError}
            </div>
          )}
          {attachedTags.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t('detail.noTags')}</p>
          ) : (
            <div className="flex flex-wrap items-center gap-1.5" data-testid="attached-tag-list">
              {attachedTags.map((tag) => (
                <TagChip
                  key={tag.id}
                  tag={tag}
                  onRemove={tagsBusy ? undefined : () => handleDetach(tag.id)}
                />
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-base">{t('library.markdownPreview')}</CardTitle>
        </CardHeader>
        <CardContent>
          {contentLoading && (
            <div className="space-y-3">
              <Skeleton className="h-4 w-3/4" />
              <Skeleton className="h-4 w-full" />
              <Skeleton className="h-4 w-5/6" />
              <Skeleton className="h-4 w-2/3" />
              <Skeleton className="h-4 w-full" />
            </div>
          )}

          {contentError && (
            <div className="flex flex-col items-center gap-3">
              <p className="text-sm text-destructive">
                {t('library.contentFailed')}{contentError.message}
              </p>
              <Button variant="outline" size="sm" onClick={loadContent}>
                {t('error.retry')}
              </Button>
            </div>
          )}

          {content && !contentLoading && !contentError && (
            <pre className="max-h-[60vh] overflow-auto whitespace-pre-wrap break-words rounded-md bg-muted p-4 text-sm leading-relaxed text-foreground font-mono">
              {content}
            </pre>
          )}
        </CardContent>
      </Card>

      {/* Add-tag picker: shows tags not yet attached. Single-click attaches and closes. */}
      <Dialog open={addTagOpen} onOpenChange={(o) => !o && setAddTagOpen(false)}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <TagIcon className="h-4 w-4" />
              {t('detail.addTag')}
            </DialogTitle>
          </DialogHeader>
          {availableTags.length === 0 ? (
            <p className="py-4 text-center text-xs text-muted-foreground">
              {t('detail.noAvailableTags')}
            </p>
          ) : (
            <div className="max-h-64 space-y-1 overflow-y-auto rounded-md border p-2">
              {availableTags.map((tag) => (
                <button
                  key={tag.id}
                  type="button"
                  disabled={tagsBusy}
                  onClick={() => {
                    // Always close the dialog on click — even if attach fails,
                    // the error is surfaced in the inline `tagError` strip
                    // below the tag list. (handleAttach is fire-and-forget.)
                    void handleAttach(tag)
                    setAddTagOpen(false)
                  }}
                  className="flex w-full items-center justify-between rounded px-2 py-1.5 text-left text-sm hover:bg-muted/50 disabled:opacity-50"
                >
                  <TagChip tag={tag} />
                  <Plus className="h-3.5 w-3.5 text-muted-foreground" />
                </button>
              ))}
            </div>
          )}
          <DialogFooter>
            <Button variant="outline" size="sm" onClick={() => setAddTagOpen(false)}>
              {t('common.close')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Full tag CRUD dialog, scoped to this skill via selectedSkillIds. */}
      {skillId && (
        <TagManagerDialog
          open={manageOpen}
          onClose={handleManageClose}
          selectedSkillIds={[skillId]}
        />
      )}
    </div>
  )
}