import { useState, useEffect, useCallback, useMemo } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { useLibrary } from '@/hooks/use-library'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { ArrowLeft, Folder, Loader2 } from 'lucide-react'

export default function SkillDetail() {
  const { skillName } = useParams<{ skillName: string }>()
  const { t } = useTranslation()
  const navigate = useNavigate()
  const { installedSkills, scanResult, scan, tools, fetchTools, getContent, linkSkill, unlinkSkill } = useLibrary()

  const [content, setContent] = useState<string | null>(null)
  const [contentLoading, setContentLoading] = useState(true)
  const [contentError, setContentError] = useState<Error | null>(null)
  const [togglingTool, setTogglingTool] = useState<string | null>(null)
  const [localLinkedTools, setLocalLinkedTools] = useState<Set<string>>(new Set())
  const [toggleError, setToggleError] = useState<string | null>(null)

  const decodedName = skillName ? decodeURIComponent(skillName) : ''

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
    </div>
  )
}