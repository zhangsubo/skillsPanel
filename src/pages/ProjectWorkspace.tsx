import { useState, useMemo, useEffect, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { useParams } from 'react-router-dom'
import { Search, RefreshCw, LayoutGrid, List, CheckSquare, Package, Plus, X, Check, Trash2 } from 'lucide-react'
import { useNavigate } from 'react-router-dom'
import { confirm } from '@tauri-apps/plugin-dialog'
import { useProjects } from '@/hooks/use-projects'
import { getInstalledSkillsFromDb } from '@/api/database'
import { exportSkillToProjectMulti, deleteProjectSkill, importProjectSkill } from '@/api/projects'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent } from '@/components/ui/card'
import { Skeleton } from '@/components/ui/skeleton'
import { AgentCheckboxGroup } from '@/components/project/AgentCheckboxGroup'
import { SkillActionMenu } from '@/components/project/SkillActionMenu'
import { EditSkillAgentsDialog } from '@/components/project/EditSkillAgentsDialog'
import type { ProjectSkillInfo, Skill } from '@/types'

type FilterMode = 'all' | 'enabled' | 'disabled'
type ViewMode = 'grid' | 'list'

export default function ProjectWorkspace() {
  const { t } = useTranslation()
  const { projectId } = useParams<{ projectId: string }>()
  const { projects, projectDetail, scanning, selectProject, removeProject } = useProjects()
  const navigate = useNavigate()

  const handleDelete = async () => {
    if (!projectId || !project) return
    const confirmed = await confirm(t('project.confirmDelete', { name: project.name }), {
      title: t('project.deleteTitle'),
      kind: 'warning',
    })
    if (!confirmed) return
    try {
      // 先导航到首页，避免删除过程中渲染问题
      navigate('/')
      // 然后删除项目（异步执行，不阻塞导航）
      removeProject(projectId).catch((err) => {
        console.error('Failed to delete project:', err)
        alert(t('project.deleteFailed', { error: String(err) }))
      })
    } catch (err) {
      console.error('Failed to delete project:', err)
      alert(t('project.deleteFailed', { error: String(err) }))
    }
  }

  const [searchQuery, setSearchQuery] = useState('')
  const [filterMode, setFilterMode] = useState<FilterMode>('all')
  const [viewMode, setViewMode] = useState<ViewMode>('grid')
  const [selectedSkills, setSelectedSkills] = useState<Set<string>>(new Set())
  const [showAddSkillDialog, setShowAddSkillDialog] = useState(false)
  const [editingSkill, setEditingSkill] = useState<{ name: string; agent: string } | null>(null)

  const project = projects.find((p) => p.id === projectId)

  useEffect(() => {
    if (projectId && projectId !== projectDetail?.project.id) {
      selectProject(projectId)
    }
  }, [projectId, projectDetail?.project.id, selectProject])

  const filteredSkills = useMemo(() => {
    if (!projectDetail) return []
    let skills = projectDetail.skills
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase()
      skills = skills.filter(
        (s) => s.name.toLowerCase().includes(q) || s.description.toLowerCase().includes(q),
      )
    }
    if (filterMode === 'enabled') skills = skills.filter((s) => s.enabled)
    if (filterMode === 'disabled') skills = skills.filter((s) => !s.enabled)
    return skills
  }, [projectDetail, searchQuery, filterMode])

  const enabledCount = projectDetail?.skills.filter((s) => s.enabled).length ?? 0
  const totalCount = projectDetail?.skills.length ?? 0

  const toggleSkillSelection = (name: string) => {
    setSelectedSkills((prev) => {
      const next = new Set(prev)
      if (next.has(name)) next.delete(name)
      else next.add(name)
      return next
    })
  }

  const handleRefresh = async () => {
    if (projectId) await selectProject(projectId)
  }

  const handleDeleteSkill = async (skillName: string, agent: string) => {
    if (!projectId) return
    const confirmed = await confirm(t('project.confirmDeleteSkill', { name: skillName }), {
      title: t('project.deleteSkillTitle'),
      kind: 'warning',
    })
    if (!confirmed) return

    try {
      await deleteProjectSkill(projectId, skillName, agent)
      alert(t('project.skillDeleted', { name: skillName, agent }))
      await handleRefresh()
    } catch (err) {
      alert(`删除失败: ${err instanceof Error ? err.message : String(err)}`)
    }
  }

  const handleImportToCenter = async (skillName: string) => {
    if (!projectId) return
    try {
      await importProjectSkill(projectId, skillName)
      alert(t('installSkill.importedOne', { name: skillName }))
      await handleRefresh()
    } catch (err) {
      alert(`导入失败: ${err instanceof Error ? err.message : String(err)}`)
    }
  }

  if (!project && !scanning) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4 p-6">
        <Package className="h-12 w-12 text-muted-foreground" />
        <p className="text-muted-foreground">{t('project.selectHint')}</p>
      </div>
    )
  }

  if (scanning && !projectDetail) {
    return (
      <div className="flex h-full flex-col gap-6 p-6">
        <Skeleton className="h-8 w-64" />
        <Skeleton className="h-4 w-48" />
        <Skeleton className="h-10 w-full max-w-sm" />
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {Array.from({ length: 4 }).map((_, i) => (
            <Card key={i} className="h-40">
              <CardContent className="p-5 space-y-3">
                <Skeleton className="h-5 w-24" />
                <Skeleton className="h-4 w-full" />
                <Skeleton className="h-4 w-3/4" />
              </CardContent>
            </Card>
          ))}
        </div>
      </div>
    )
  }

  return (
    <div className="flex h-full flex-col gap-6 p-6">
      {/* ── Header ─────────────────────────────────────────── */}
      <div className="flex items-start justify-between">
        <div>
          <h2 className="text-2xl font-semibold text-foreground">
            {project?.name ?? '...'}
            <Badge variant="secondary" className="ml-2 align-middle text-xs">
              {enabledCount}
            </Badge>
          </h2>
          <p className="mt-1 text-sm text-muted-foreground">
            {project?.root_path} · {enabledCount} / {totalCount} {t('project.enabled')}
          </p>
          <p className="mt-0.5 text-sm text-muted-foreground">
            {t('project.description')}
          </p>
        </div>
        <Button variant="outline" size="sm" onClick={() => setShowAddSkillDialog(true)}>
          <Plus className="mr-1 h-3.5 w-3.5" />
          {t('project.addSkill')}
        </Button>
      </div>

      {/* ── Toolbar ────────────────────────────────────────── */}
      <div className="flex items-center gap-3">
        <div className="relative max-w-sm flex-1">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            placeholder={t('project.searchPlaceholder')}
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="pl-9"
          />
        </div>

        {/* Filter tabs */}
        <div className="flex items-center rounded-md border border-gray-200 dark:border-gray-700">
          {(['all', 'enabled', 'disabled'] as const).map((mode) => (
            <button
              key={mode}
              onClick={() => setFilterMode(mode)}
              className={[
                'px-3 py-1.5 text-xs font-medium transition-colors',
                filterMode === mode
                  ? 'bg-gray-100 text-gray-900 dark:bg-gray-700 dark:text-gray-100'
                  : 'text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200',
              ].join(' ')}
            >
              {t(`project.filter.${mode}`)}
            </button>
          ))}
        </div>

        {/* View actions */}
        <div className="flex items-center gap-1">
          <Button variant="ghost" size="icon" className="h-8 w-8" onClick={handleRefresh}>
            <RefreshCw className={`h-4 w-4 ${scanning ? 'animate-spin' : ''}`} />
          </Button>
          <Button variant="ghost" size="icon" className="h-8 w-8 text-destructive hover:text-destructive" onClick={handleDelete} title={t('project.delete')}>
            <Trash2 className="h-4 w-4" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="h-8 w-8"
            onClick={() => setViewMode('grid')}
          >
            <LayoutGrid className="h-4 w-4" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="h-8 w-8"
            onClick={() => setViewMode('list')}
          >
            <List className="h-4 w-4" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="h-8 w-8"
            onClick={() => {
              if (selectedSkills.size === filteredSkills.length) {
                setSelectedSkills(new Set())
              } else {
                setSelectedSkills(new Set(filteredSkills.map((s) => s.name)))
              }
            }}
          >
            <CheckSquare className="h-4 w-4" />
          </Button>
        </div>
      </div>

      {/* ── Skills List ────────────────────────────────────── */}
      {filteredSkills.length === 0 ? (
        <div className="flex flex-1 items-center justify-center">
          <p className="text-muted-foreground">{t('project.noSkills')}</p>
        </div>
      ) : viewMode === 'grid' ? (
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {filteredSkills.map((skill) => (
            <ProjectSkillCard
              key={skill.name}
              skill={skill}
              selected={selectedSkills.has(skill.name)}
              onSelect={() => toggleSkillSelection(skill.name)}
              onDeleteSkill={handleDeleteSkill}
              onEditAgents={(name, agent) => setEditingSkill({ name, agent })}
              onImportToCenter={handleImportToCenter}
            />
          ))}
        </div>
      ) : (
        <div className="space-y-2">
          {filteredSkills.map((skill) => (
            <ProjectSkillRow
              key={skill.name}
              skill={skill}
              selected={selectedSkills.has(skill.name)}
              onSelect={() => toggleSkillSelection(skill.name)}
              onDeleteSkill={handleDeleteSkill}
              onEditAgents={(name, agent) => setEditingSkill({ name, agent })}
              onImportToCenter={handleImportToCenter}
            />
          ))}
        </div>
      )}

      {showAddSkillDialog && projectId && (
        <AddSkillToProjectDialog
          projectId={projectId}
          existingSkills={projectDetail?.skills.map((s) => s.name) ?? []}
          onClose={() => setShowAddSkillDialog(false)}
          onAdded={() => {
            setShowAddSkillDialog(false)
            handleRefresh()
          }}
        />
      )}

      {editingSkill && projectId && (
        <EditSkillAgentsDialog
          projectId={projectId}
          skillName={editingSkill.name}
          currentAgent={editingSkill.agent}
          onClose={() => setEditingSkill(null)}
          onUpdated={() => {
            setEditingSkill(null)
            handleRefresh()
          }}
        />
      )}
    </div>
  )
}

function getSyncStatusLabel(status: ProjectSkillInfo['sync_status']): string {
  const map: Record<string, string> = {
    in_sync: '已同步',
    center_newer: '中心更新',
    project_newer: '项目更新',
    diverged: '已分歧',
    project_only: '仅项目',
    center_only: '仅中心',
  }
  return map[status] ?? status
}

function getSyncStatusColor(status: ProjectSkillInfo['sync_status']): string {
  const map: Record<string, string> = {
    in_sync: 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400',
    center_newer: 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400',
    project_newer: 'bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400',
    diverged: 'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400',
    project_only: 'bg-gray-100 text-gray-600 dark:bg-gray-700 dark:text-gray-400',
    center_only: 'bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-400',
  }
  return map[status] ?? ''
}

function getAgentBadgeClass(agent: string): string {
  const map: Record<string, string> = {
    'claude-code': 'bg-indigo-100 text-indigo-700 dark:bg-indigo-900/30 dark:text-indigo-400',
    cursor: 'bg-sky-100 text-sky-700 dark:bg-sky-900/30 dark:text-sky-400',
    opencode: 'bg-teal-100 text-teal-700 dark:bg-teal-900/30 dark:text-teal-400',
    codex: 'bg-orange-100 text-orange-700 dark:bg-orange-900/30 dark:text-orange-400',
  }
  return map[agent] ?? 'bg-gray-100 text-gray-600 dark:bg-gray-700 dark:text-gray-400'
}

function ProjectSkillCard({
  skill,
  selected,
  onSelect,
  onDeleteSkill,
  onEditAgents,
  onImportToCenter,
}: {
  skill: ProjectSkillInfo
  selected: boolean
  onSelect: () => void
  onDeleteSkill: (name: string, agent: string) => void
  onEditAgents: (name: string, agent: string) => void
  onImportToCenter: (name: string) => void
}) {
  const { t } = useTranslation()
  return (
    <Card
      className={`group relative cursor-pointer transition-all hover:shadow-md ${
        selected ? 'border-primary ring-1 ring-primary/30' : 'hover:border-primary/30'
      }`}
      onClick={onSelect}
    >
      <CardContent className="p-5">
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0 flex-1">
            <h3 className="truncate text-base font-semibold text-foreground">{skill.name}</h3>
          </div>
          <div className="flex items-center gap-1 shrink-0">
            <span className={`inline-flex h-5 items-center rounded-full px-1.5 text-[10px] font-medium ${getSyncStatusColor(skill.sync_status)}`}>
              {getSyncStatusLabel(skill.sync_status)}
            </span>
            <SkillActionMenu
              skillName={skill.name}
              agent={skill.agent}
              onEditAgents={() => onEditAgents(skill.name, skill.agent)}
              onDelete={() => onDeleteSkill(skill.name, skill.agent)}
              onImportToCenter={() => onImportToCenter(skill.name)}
            />
          </div>
        </div>

        {skill.description && (
          <p className="mt-2 line-clamp-2 text-sm text-muted-foreground">{skill.description}</p>
        )}

        <div className="mt-3 flex items-center gap-1.5 flex-wrap">
          <span className="inline-flex h-6 items-center rounded-full bg-muted px-2 text-xs font-medium text-muted-foreground">
            {t('project.scopeProject')}
          </span>
          <span className={`inline-flex h-6 items-center rounded-full px-2 text-xs font-medium ${getAgentBadgeClass(skill.agent)}`}>
            {skill.agent}
          </span>
        </div>

        {skill.enabled && (
          <div className="mt-2 text-xs text-green-600 dark:text-green-400">
            {t('project.syncEnabled')}
          </div>
        )}
      </CardContent>
    </Card>
  )
}

function ProjectSkillRow({
  skill,
  selected,
  onSelect,
  onDeleteSkill,
  onEditAgents,
  onImportToCenter,
}: {
  skill: ProjectSkillInfo
  selected: boolean
  onSelect: () => void
  onDeleteSkill: (name: string, agent: string) => void
  onEditAgents: (name: string, agent: string) => void
  onImportToCenter: (name: string) => void
}) {
  const { t } = useTranslation()
  return (
    <div
      onClick={onSelect}
      className={`flex items-center gap-4 rounded-lg border p-4 transition-all cursor-pointer hover:shadow-sm ${
        selected ? 'border-primary bg-primary/5' : 'border-gray-200 hover:border-primary/30 dark:border-gray-700'
      }`}
    >
      <Package className="h-5 w-5 shrink-0 text-primary" />
      <div className="min-w-0 flex-1">
        <h3 className="text-sm font-semibold text-foreground">{skill.name}</h3>
        {skill.description && (
          <p className="mt-0.5 line-clamp-1 text-xs text-muted-foreground">{skill.description}</p>
        )}
      </div>
      <span className="inline-flex h-6 items-center rounded-full bg-muted px-2 text-xs font-medium text-muted-foreground">
        {t('project.scopeProject')}
      </span>
      <span className={`inline-flex h-6 items-center rounded-full px-2 text-xs font-medium ${getAgentBadgeClass(skill.agent)}`}>
        {skill.agent}
      </span>
      <span className={`inline-flex h-5 items-center rounded-full px-1.5 text-[10px] font-medium ${getSyncStatusColor(skill.sync_status)}`}>
        {getSyncStatusLabel(skill.sync_status)}
      </span>
      <SkillActionMenu
        skillName={skill.name}
        agent={skill.agent}
        onEditAgents={() => onEditAgents(skill.name, skill.agent)}
        onDelete={() => onDeleteSkill(skill.name, skill.agent)}
        onImportToCenter={() => onImportToCenter(skill.name)}
      />
    </div>
  )
}

function AddSkillToProjectDialog({
  projectId,
  existingSkills,
  onClose,
  onAdded,
}: {
  projectId: string
  existingSkills: string[]
  onClose: () => void
  onAdded: () => void
}) {
  const { t } = useTranslation()
  const [skills, setSkills] = useState<Skill[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [searchQuery, setSearchQuery] = useState('')
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [submitting, setSubmitting] = useState(false)
  const [targetAgents, setTargetAgents] = useState<string[]>(['claude-code'])

  useEffect(() => {
    let cancelled = false
    ;(async () => {
      try {
        const dbSkills = await getInstalledSkillsFromDb()
        if (!cancelled) setSkills(dbSkills)
      } catch {
        if (!cancelled) setError('Failed to load skills')
      } finally {
        if (!cancelled) setLoading(false)
      }
    })()
    return () => { cancelled = true }
  }, [])

  const filtered = useMemo(() => {
    const existing = new Set(existingSkills)
    let list = skills.filter((s) => !existing.has(s.name))
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase()
      list = list.filter(
        (s) => s.name.toLowerCase().includes(q) || s.description.toLowerCase().includes(q),
      )
    }
    return list
  }, [skills, existingSkills, searchQuery])

  const toggle = useCallback((name: string) => {
    setSelected((prev) => {
      const next = new Set(prev)
      if (next.has(name)) next.delete(name)
      else next.add(name)
      return next
    })
  }, [])

  const handleSubmit = async () => {
    if (selected.size === 0 || targetAgents.length === 0) return
    setSubmitting(true)
    setError(null)
    try {
      for (const name of selected) {
        await exportSkillToProjectMulti(projectId, name, targetAgents)
      }
      onAdded()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="flex h-[500px] w-[500px] flex-col rounded-lg bg-white p-6 shadow-xl dark:bg-gray-900">
        <div className="flex items-center justify-between">
          <h3 className="text-lg font-semibold text-foreground">{t('project.addSkill')}</h3>
          <button onClick={onClose} className="rounded p-1 text-muted-foreground hover:bg-muted">
            <X className="h-4 w-4" />
          </button>
        </div>
        <p className="mt-1 text-sm text-muted-foreground">
          {t('project.addSkillDesc')}
        </p>

        <div className="mt-4 space-y-3">
          <div>
            <label className="mb-1.5 block text-xs font-medium text-foreground">
              {t('project.targetAgents')}
            </label>
            <AgentCheckboxGroup
              value={targetAgents}
              onChange={setTargetAgents}
              disabled={submitting}
            />
          </div>

          <div className="relative">
            <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              placeholder={t('project.searchPlaceholder')}
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="pl-9"
            />
          </div>
        </div>

        <div className="mt-3 flex-1 overflow-y-auto">
          {loading ? (
            <div className="space-y-2">
              {Array.from({ length: 4 }).map((_, i) => (
                <Skeleton key={i} className="h-14 w-full" />
              ))}
            </div>
          ) : error ? (
            <p className="py-4 text-center text-sm text-red-600">{error}</p>
          ) : filtered.length === 0 ? (
            <p className="py-4 text-center text-sm text-muted-foreground">
              {t('project.noSkills')}
            </p>
          ) : (
            <div className="space-y-1">
              {filtered.map((skill) => (
                <button
                  key={skill.name}
                  onClick={() => toggle(skill.name)}
                  className={`flex w-full items-center gap-3 rounded-md px-3 py-2.5 text-left text-sm transition-colors ${
                    selected.has(skill.name)
                      ? 'bg-primary/10 text-primary'
                      : 'hover:bg-muted text-foreground'
                  }`}
                >
                  <div className="flex h-5 w-5 shrink-0 items-center justify-center rounded border">
                    {selected.has(skill.name) && <Check className="h-3.5 w-3.5" />}
                  </div>
                  <div className="min-w-0 flex-1">
                    <p className="font-medium">{skill.name}</p>
                    {skill.description && (
                      <p className="mt-0.5 truncate text-xs text-muted-foreground">
                        {skill.description}
                      </p>
                    )}
                  </div>
                </button>
              ))}
            </div>
          )}
        </div>

        {error && !loading && (
          <p className="mt-2 text-sm text-red-600 dark:text-red-400">{error}</p>
        )}

        <div className="mt-4 flex items-center justify-between border-t pt-4">
          <span className="text-xs text-muted-foreground">
            {selected.size} {t('project.selected')}
          </span>
          <div className="flex gap-2">
            <button
              onClick={onClose}
              className="rounded-md px-3 py-1.5 text-sm text-muted-foreground hover:bg-muted"
            >
              {t('library.cancel')}
            </button>
            <button
              onClick={handleSubmit}
              disabled={submitting || selected.size === 0}
              className="rounded-md bg-gray-900 px-3 py-1.5 text-sm text-white hover:bg-gray-800 disabled:opacity-50 dark:bg-gray-100 dark:text-gray-900 dark:hover:bg-gray-200"
            >
              {submitting ? '...' : t('project.addProject')}
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}
