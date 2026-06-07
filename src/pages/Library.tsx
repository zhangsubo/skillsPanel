import { useState, useEffect, useMemo } from 'react'
import { useNavigate } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { useLibrary } from '@/hooks/use-library'
import { useTags } from '@/hooks/use-tags'
import { Card, CardContent } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog'
import { Package, Trash2, Search, Loader2, Tag as TagIcon } from 'lucide-react'
import type { Skill, SkillWithStatus, Tag } from '@/types'
import { batchDeleteSkills } from '@/api/library'
import { TagChip } from '@/components/TagChip'
import { TagFilter } from '@/components/TagFilter'
import { TagManagerDialog } from '@/components/TagManagerDialog'

const createFallbackSkill = (name: string): Skill => ({
  id: name,
  name,
  path_hash: name,
  library_path: '',
  original_source_path: null,
  original_git_url: null,
  original_git_subpath: null,
  group: 'library',
  description: 'No description',
  frontmatter: {},
  created_at: '',
  mtime_ms: 0,
  source_type: 'local-folder',
  is_deleted: false,
  source_revision: null,
  source_remote_revision: null,
  source_update_status: 'up-to-date',
})

export default function Library() {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const { skillNames, installedSkills, scanResult, loading, error, refresh, scan, deleteSkill } = useLibrary()
  const { tags, fetchAllSkillTagsMap } = useTags()
  const [searchQuery, setSearchQuery] = useState('')
  const [filterTagId, setFilterTagId] = useState<string | null>(null)
  const [skillTagMap, setSkillTagMap] = useState<Map<string, Tag[]>>(new Map())
  const [tagManagerOpen, setTagManagerOpen] = useState(false)
  const [deleteTarget, setDeleteTarget] = useState<SkillWithStatus | null>(null)
  const [deleting, setDeleting] = useState(false)
  const [clearingAll, setClearingAll] = useState(false)

  useEffect(() => {
    if (!scanResult) {
      scan()
    }
  }, [scanResult, scan])

  const skills = useMemo(() => {
    const installedSkillNames = installedSkills.length > 0
      ? installedSkills.map((skill) => skill.name)
      : skillNames
    if (installedSkillNames.length === 0) {
      return []
    }

    const scanSkillsByName = new Map(
      scanResult?.skills.map((skillWithStatus) => [skillWithStatus.skill.name, skillWithStatus]) ?? [],
    )
    const installedSkillsByName = new Map(installedSkills.map((skill) => [skill.name, skill]))

    return Array.from(new Set(installedSkillNames)).map((name) => {
      const scannedSkill = scanSkillsByName.get(name)
      if (scannedSkill) {
        return scannedSkill
      }

      const installedSkill = installedSkillsByName.get(name)
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

      return {
        skill: createFallbackSkill(name),
        tool_statuses: {},
        rule_decisions: {},
      }
    })
  }, [installedSkills, scanResult, skillNames])

  const filteredSkills = useMemo(() => {
    let result = skills
    if (filterTagId) {
      result = result.filter((s) => (skillTagMap.get(s.skill.id) ?? []).some((t) => t.id === filterTagId))
    }
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase()
      result = result.filter(
        (s) =>
          s.skill.name.toLowerCase().includes(q) ||
          s.skill.description.toLowerCase().includes(q),
      )
    }
    return result
  }, [skills, searchQuery, filterTagId, skillTagMap])

  // Load skill→tags mapping whenever the visible skills change.
  useEffect(() => {
    let cancelled = false
    void (async () => {
      try {
        const map = await fetchAllSkillTagsMap()
        if (!cancelled) setSkillTagMap(map)
      } catch {
        // best-effort: leave the map empty on error
      }
    })()
    return () => {
      cancelled = true
    }
  }, [fetchAllSkillTagsMap, skills])

  const handleDelete = async () => {
    if (!deleteTarget) return
    setDeleting(true)
    try {
      await deleteSkill(deleteTarget.skill.name)
      await refresh()
      await scan()
      setDeleteTarget(null)
    } catch {
      // Error handled by hook
    } finally {
      setDeleting(false)
    }
  }

  const handleClearAll = async () => {
    if (skillNames.length === 0) return
    setClearingAll(true)
    try {
      await batchDeleteSkills(skillNames, true)
      await refresh()
      await scan()
    } catch {
      // Error handled by hook
    } finally {
      setClearingAll(false)
    }
  }

  // Get linked tool names for a skill
  const getLinkedToolNames = (skill: SkillWithStatus): string[] => {
    return Object.entries(skill.tool_statuses)
      .filter(([, status]) => status === 'linked')
      .map(([toolName]) => toolName)
  }

  // Get initials from tool name for badge display
  const getToolInitials = (name: string): string => {
    const parts = name.replace(/[-_]/g, ' ').split(' ')
    if (parts.length >= 2) {
      return (parts[0][0] + parts[1][0]).toUpperCase()
    }
    return name.slice(0, 2).toUpperCase()
  }

  // Loading state
  if (loading && skills.length === 0) {
    return (
      <div className="flex h-full flex-col gap-6 p-6">
        <div className="flex items-center justify-between">
          <Skeleton className="h-8 w-48" />
          <Skeleton className="h-6 w-20" />
        </div>
        <Skeleton className="h-10 w-full max-w-sm" />
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {Array.from({ length: 6 }).map((_, i) => (
            <Card key={i} className="h-44">
              <CardContent className="p-5 space-y-3">
                <Skeleton className="h-5 w-24" />
                <Skeleton className="h-4 w-full" />
                <Skeleton className="h-4 w-3/4" />
                <div className="flex gap-1.5 pt-2">
                  <Skeleton className="h-6 w-8 rounded-full" />
                  <Skeleton className="h-6 w-8 rounded-full" />
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      </div>
    )
  }

  // Error state
  if (error && skills.length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4 p-6">
        <p className="text-sm text-destructive">
          {t('library.loadFailed')}{error.message}
        </p>
        <Button variant="outline" onClick={refresh}>
          {t('error.retry')}
        </Button>
      </div>
    )
  }

  // Empty state
  if (skills.length === 0 && !loading) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4 p-6">
        <Package className="h-12 w-12 text-muted-foreground" />
        <p className="text-muted-foreground">
          {t('library.empty')}
        </p>
        <Button variant="outline" onClick={() => navigate('/scanner')}>
          {t('nav.scanner')}
        </Button>
      </div>
    )
  }

  return (
    <div className="flex h-full flex-col gap-6 p-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-semibold text-foreground">{t('library.title')}</h2>
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => setTagManagerOpen(true)}
            className="text-xs"
          >
            <TagIcon className="mr-1 h-3.5 w-3.5" />
            {t('tag.manage')}
          </Button>
          {skills.length > 0 && (
            <Button
              variant="outline"
              size="sm"
              onClick={handleClearAll}
              disabled={clearingAll}
              className="text-xs"
            >
              {clearingAll ? (
                <Loader2 className="mr-1 h-3.5 w-3.5 animate-spin" />
              ) : (
                <Trash2 className="mr-1 h-3.5 w-3.5" />
              )}
              {t('settings.clearAll')}
            </Button>
          )}
          <Badge variant="secondary">{t('library.skillsCount', { count: filteredSkills.length })}</Badge>
        </div>
      </div>

      {/* Search */}
      <div className="flex flex-col gap-3">
        <div className="relative max-w-sm">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            placeholder={t('library.searchPlaceholder')}
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="pl-9"
          />
        </div>
        {tags.length > 0 && (
          <TagFilter
            tags={tags}
            value={filterTagId}
            onChange={setFilterTagId}
          />
        )}
      </div>

      {/* Card Grid */}
      {filteredSkills.length === 0 ? (
        <div className="flex flex-1 items-center justify-center">
          <p className="text-muted-foreground">{t('library.noMatch')}</p>
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {filteredSkills.map((skillWithStatus) => {
            const { skill } = skillWithStatus
            const linkedTools = getLinkedToolNames(skillWithStatus)
            const displayTools = linkedTools.slice(0, 3)
            const extraCount = linkedTools.length - 3

            return (
              <Card
                key={skill.id}
                className="group relative cursor-pointer transition-all hover:shadow-md hover:border-primary/30 dark:hover:border-primary/40"
                onClick={() => navigate(`/library/${encodeURIComponent(skill.name)}`)}
              >
                <CardContent className="p-5">
                  <div className="flex items-start justify-between gap-2">
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <Package className="h-4 w-4 shrink-0 text-primary" />
                        <h3 className="truncate text-base font-semibold text-foreground">
                          {skill.name}
                        </h3>
                      </div>
                    </div>
                  </div>

                  {skill.description && (
                    <p className="mt-2 line-clamp-2 text-sm text-muted-foreground">
                      {skill.description}
                    </p>
                  )}

                  {/* Agent badges */}
                  {linkedTools.length > 0 && (
                    <div className="mt-3 flex items-center gap-1.5 flex-wrap">
                      {displayTools.map((toolName) => (
                        <span
                          key={toolName}
                          className="inline-flex h-6 items-center rounded-full bg-primary/10 px-2 text-xs font-medium text-primary dark:bg-primary/20"
                        >
                          {getToolInitials(toolName)}
                        </span>
                      ))}
                      {extraCount > 0 && (
                        <span className="inline-flex h-6 items-center rounded-full bg-muted px-2 text-xs font-medium text-muted-foreground">
                          +{extraCount}
                        </span>
                      )}
                    </div>
                  )}

                  {/* Tag chips — read from the bulk skill→tags map.
                      Clicking a chip applies the corresponding tag filter. */}
                  {(() => {
                    const skillTags = skillTagMap.get(skill.id) ?? []
                    if (skillTags.length === 0) return null
                    return (
                      <div className="mt-2 flex items-center gap-1 flex-wrap">
                        {skillTags.slice(0, 4).map((tag) => (
                          <TagChip
                            key={tag.id}
                            tag={tag}
                            onClick={(t) => setFilterTagId(t.id)}
                            selected={filterTagId === tag.id}
                          />
                        ))}
                        {skillTags.length > 4 && (
                          <span className="text-[10px] text-muted-foreground">
                            +{skillTags.length - 4}
                          </span>
                        )}
                      </div>
                    )
                  })()}

                  {/* Delete button - visible on hover */}
                  <div className="absolute bottom-3 right-3 opacity-0 transition-opacity group-hover:opacity-100">
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-8 w-8 text-muted-foreground hover:text-destructive"
                      onClick={(e) => {
                        e.stopPropagation()
                        setDeleteTarget(skillWithStatus)
                      }}
                    >
                      <Trash2 className="h-4 w-4" />
                    </Button>
                  </div>
                </CardContent>
              </Card>
            )
          })}
        </div>
      )}

      {/* Delete Confirmation Dialog */}
      <Dialog open={!!deleteTarget} onOpenChange={(open) => !open && setDeleteTarget(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t('library.deleteTitle')}</DialogTitle>
            <DialogDescription>
              {t('library.deleteConfirm', { name: deleteTarget?.skill.name ?? '' })}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setDeleteTarget(null)}
              disabled={deleting}
            >
              {t('library.cancel')}
            </Button>
            <Button
              variant="destructive"
              onClick={handleDelete}
              disabled={deleting}
            >
              {deleting ? '...' : t('library.deleteTitle')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <TagManagerDialog
        open={tagManagerOpen}
        onClose={() => setTagManagerOpen(false)}
      />
    </div>
  )
}
