import { useState, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import {
  installLocalSkill,
  installGitSkill,
} from '@/api/install'
import { scanSkills, getScanDiff, getLibrary } from '@/api/library'
import type { SkillWithStatus } from '@/types'
import type { ScanDiff } from '@/api/library'
import {
  Tabs,
  TabsList,
  TabsTrigger,
  TabsContent,
} from '@/components/ui/tabs'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Upload,
  ScanLine,
  Check,
  Loader2,
  AlertCircle,
} from 'lucide-react'
import { open } from '@tauri-apps/plugin-dialog'
import { getCurrentWebview } from '@tauri-apps/api/webview'

const isTauriEnv = (): boolean => {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

const getSelectedPathKind = (path: string): 'folder' | 'zip' => {
  return path.toLowerCase().endsWith('.zip') ? 'zip' : 'folder'
}

function LocalInstallTab() {
  const { t } = useTranslation()
  const [isDragOver, setIsDragOver] = useState(false)
  const [droppedPath, setDroppedPath] = useState<string | null>(null)
  const [selectedPathKind, setSelectedPathKind] = useState<'folder' | 'zip' | null>(null)
  const [skillName, setSkillName] = useState('')
  const [installing, setInstalling] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [success, setSuccess] = useState<string | null>(null)

  const setSelectedPath = (path: string) => {
    setDroppedPath(path)
    setSelectedPathKind(getSelectedPathKind(path))
  }

  useEffect(() => {
    if (!isTauriEnv()) return

    let unlisten: (() => void) | undefined

    const setupDragDrop = async () => {
      try {
        const webview = getCurrentWebview()
        unlisten = await webview.onDragDropEvent((event) => {
          const payload = event.payload
          if (payload.type === 'enter') {
            setIsDragOver(true)
          } else if (payload.type === 'leave') {
            setIsDragOver(false)
          } else if (payload.type === 'drop') {
            setIsDragOver(false)
            const paths = payload.paths
            if (paths.length > 0) {
              setSelectedPath(paths[0])
            }
          }
        })
      } catch (err) {
        console.warn('Failed to setup drag-drop listener:', err)
      }
    }

    setupDragDrop()

    return () => {
      if (unlisten) unlisten()
    }
  }, [])

  const handlePick = async () => {
    if (!isTauriEnv()) {
      setError('文件对话框仅在桌面客户端可用')
      return
    }
    try {
      // Open folder dialog - users can drag-drop zip files instead
      const selected = await open({
        directory: true,
        multiple: false,
      })
      if (selected && typeof selected === 'string') {
        setSelectedPath(selected)
      }
    } catch (err) {
      console.error('Failed to open folder dialog:', err)
    }
  }

  const handleInstall = async () => {
    if (!droppedPath) return
    setInstalling(true)
    setError(null)
    setSuccess(null)
    try {
      const path = droppedPath
      await installLocalSkill(path, skillName.trim() || undefined)
      setSuccess(t('installSkill.localSuccess', { name: skillName || droppedPath }))
      setDroppedPath(null)
      setSelectedPathKind(null)
      setSkillName('')
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setInstalling(false)
    }
  }

  return (
    <div className="space-y-6">
      <div
        onClick={handlePick}
        className={`
          flex cursor-pointer flex-col items-center justify-center gap-3 rounded-xl border-2 border-dashed
          p-10 transition-colors
          ${isDragOver
            ? 'border-blue-500 bg-blue-500/5'
            : 'border-border bg-card hover:border-muted-foreground'
          }
        `}
      >
        <div className="flex h-14 w-14 items-center justify-center rounded-full bg-muted">
          <Upload className="h-6 w-6 text-muted-foreground" />
        </div>
        <p className="text-sm text-muted-foreground">
          {t('installSkill.dragHint')}
        </p>
        <p className="text-xs text-muted-foreground">
          {t('installSkill.dragFormats')}
        </p>
        <p className="text-xs text-muted-foreground mt-1">
          {t('installSkill.dragZipHint')}
        </p>
      </div>

      {droppedPath && (
        <div className="flex items-center gap-2 rounded-lg bg-muted px-4 py-3 text-sm">
          <Check className="h-4 w-4 text-green-600" />
          <span className="flex-1 truncate">{droppedPath}</span>
          <Badge variant="secondary" className="shrink-0" data-testid="selected-path-kind">
            {selectedPathKind === 'zip' ? 'ZIP' : 'Folder'}
          </Badge>
          <button
            onClick={() => {
              setDroppedPath(null)
              setSelectedPathKind(null)
            }}
            className="text-xs text-muted-foreground hover:text-foreground"
          >
            {t('installSkill.clear')}
          </button>
        </div>
      )}

      <div className="space-y-2">
        <label className="text-sm text-muted-foreground">
          {t('installSkill.skillName')}
          <span className="ml-1 text-xs text-muted-foreground">({t('installSkill.optional')})</span>
        </label>
        <Input
          placeholder={t('installSkill.namePlaceholder')}
          value={skillName}
          onChange={(e) => setSkillName(e.target.value)}
        />
        <p className="text-xs text-muted-foreground">{t('installSkill.nameHint')}</p>
      </div>

      <Button
        onClick={handleInstall}
        disabled={!droppedPath || installing}
        className="w-full"
      >
        {installing ? (
          <>
            <Loader2 className="mr-2 h-4 w-4 animate-spin" />
            {t('installSkill.installing')}
          </>
        ) : (
          t('installSkill.installBtn')
        )}
      </Button>

      {error && (
        <div className="flex items-center gap-2 rounded-lg bg-red-50 px-4 py-3 text-sm text-red-600">
          <AlertCircle className="h-4 w-4 shrink-0" />
          {error}
        </div>
      )}
      {success && (
        <div className="flex items-center gap-2 rounded-lg bg-green-50 px-4 py-3 text-sm text-green-600">
          <Check className="h-4 w-4 shrink-0" />
          {success}
        </div>
      )}
    </div>
  )
}

function GitInstallTab() {
  const { t } = useTranslation()
  const [gitUrl, setGitUrl] = useState('')
  const [subpath, setSubpath] = useState('')
  const [skillName, setSkillName] = useState('')
  const [installing, setInstalling] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [success, setSuccess] = useState<string | null>(null)

  const handleInstall = async () => {
    if (!gitUrl.trim()) return
    setInstalling(true)
    setError(null)
    setSuccess(null)
    try {
      const url = gitUrl.trim()
      const sub = subpath.trim() || undefined
      const name = skillName.trim() || undefined
      await installGitSkill(url, sub, name)
      setSuccess(t('installSkill.gitSuccess', { url }))
      setGitUrl('')
      setSubpath('')
      setSkillName('')
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setInstalling(false)
    }
  }

  return (
    <div className="space-y-6">
      <div className="space-y-2">
        <label className="text-sm text-muted-foreground">
          {t('installSkill.gitUrlLabel')}
        </label>
        <Input
          placeholder="https://github.com/user/repo 或 user/repo"
          value={gitUrl}
          onChange={(e) => setGitUrl(e.target.value)}
        />
        <div className="text-xs text-muted-foreground space-y-1">
          <p>{t('installSkill.gitFormats')}:</p>
          <p>{t('installSkill.gitFormat1')}</p>
          <p>{t('installSkill.gitFormat2')}</p>
          <p>{t('installSkill.gitFormat3')}</p>
        </div>
      </div>

      <div className="space-y-2">
        <label className="text-sm text-muted-foreground">
          {t('installSkill.subpathLabel')}
          <span className="ml-1 text-xs text-muted-foreground">({t('installSkill.optional')})</span>
        </label>
        <Input
          placeholder={t('installSkill.subpathPlaceholder')}
          value={subpath}
          onChange={(e) => setSubpath(e.target.value)}
        />
        <p className="text-xs text-muted-foreground">{t('installSkill.subpathHint')}</p>
      </div>

      <div className="space-y-2">
        <label className="text-sm text-muted-foreground">
          {t('installSkill.skillName')}
          <span className="ml-1 text-xs text-muted-foreground">({t('installSkill.optional')})</span>
        </label>
        <Input
          placeholder={t('installSkill.namePlaceholder')}
          value={skillName}
          onChange={(e) => setSkillName(e.target.value)}
        />
        <p className="text-xs text-muted-foreground">{t('installSkill.nameHint')}</p>
      </div>

      <Button
        onClick={handleInstall}
        disabled={!gitUrl.trim() || installing}
        className="w-full"
      >
        {installing ? (
          <>
            <Loader2 className="mr-2 h-4 w-4 animate-spin" />
            {t('installSkill.installing')}
          </>
        ) : (
          t('installSkill.installBtn')
        )}
      </Button>

      {error && (
        <div className="flex items-center gap-2 rounded-lg bg-red-50 px-4 py-3 text-sm text-red-600">
          <AlertCircle className="h-4 w-4 shrink-0" />
          {error}
        </div>
      )}
      {success && (
        <div className="flex items-center gap-2 rounded-lg bg-green-50 px-4 py-3 text-sm text-green-600">
          <Check className="h-4 w-4 shrink-0" />
          {success}
        </div>
      )}
    </div>
  )
}

function ScanLocalTab() {
  const { t } = useTranslation()
  const [scanning, setScanning] = useState(false)
  const [skills, setSkills] = useState<SkillWithStatus[] | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set())
  const [installedIds, setInstalledIds] = useState<Set<string>>(new Set())
  const [importing, setImporting] = useState(false)
  const [resultMsg, setResultMsg] = useState<string | null>(null)
  const [scanDiff, setScanDiff] = useState<ScanDiff | null>(null)
  const [libraryNames, setLibraryNames] = useState<Set<string>>(new Set())

  const handleScan = async () => {
    setScanning(true)
    setError(null)
    setResultMsg(null)
    setSelectedIds(new Set())
    setScanDiff(null)
    try {
      const [result, libNames] = await Promise.all([
        scanSkills(),
        getLibrary(),
      ])
      setSkills(result.skills)
      setLibraryNames(new Set(libNames))
      const diff = await getScanDiff()
      setScanDiff(diff)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
      setSkills(null)
      setLibraryNames(new Set())
      setScanDiff(null)
    } finally {
      setScanning(false)
    }
  }

  const toggleSelect = (id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  const isInLibrary = (skill: SkillWithStatus) =>
    libraryNames.has(skill.skill.name)

  const toggleSelectAll = () => {
    if (!skills) return
    const selectable = skills
      .filter((s) => !installedIds.has(s.skill.id) && !isInLibrary(s))
      .map((s) => s.skill.id)
    const allSelected = selectable.every((id) => selectedIds.has(id))
    if (allSelected) {
      setSelectedIds(new Set())
    } else {
      setSelectedIds(new Set(selectable))
    }
  }

  const handleImportOne = async (skill: SkillWithStatus) => {
    if (isInLibrary(skill)) {
      setInstalledIds((prev) => new Set(prev).add(skill.skill.id))
      return
    }
    setImporting(true)
    setError(null)
    try {
      const sourcePath = skill.skill.original_source_path || skill.skill.library_path
      await installLocalSkill(sourcePath, skill.skill.name)
      setInstalledIds((prev) => new Set(prev).add(skill.skill.id))
      setSelectedIds((prev) => {
        const next = new Set(prev)
        next.delete(skill.skill.id)
        return next
      })
      setResultMsg(t('installSkill.importedOne', { name: skill.skill.name }))
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setImporting(false)
    }
  }

  const handleImportSelected = async () => {
    if (!skills || selectedIds.size === 0) return
    setImporting(true)
    setError(null)
    let count = 0
    try {
      for (const skill of skills) {
        if (!selectedIds.has(skill.skill.id)) continue
        if (installedIds.has(skill.skill.id)) continue
        if (isInLibrary(skill)) {
          setInstalledIds((prev) => new Set(prev).add(skill.skill.id))
          continue
        }
        const sourcePath = skill.skill.original_source_path || skill.skill.library_path
        await installLocalSkill(sourcePath, skill.skill.name)
        setInstalledIds((prev) => new Set(prev).add(skill.skill.id))
        count++
      }
      setSelectedIds(new Set())
      setResultMsg(t('installSkill.importedCount', { count }))
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setImporting(false)
    }
  }

      if (!skills && !scanning) {
    return (
      <div className="flex flex-col items-center justify-center gap-6 py-16">
        <div className="flex h-20 w-20 items-center justify-center rounded-full bg-muted">
          <ScanLine className="h-10 w-10 text-muted-foreground" />
        </div>
        <p className="text-sm text-muted-foreground">
          {t('installSkill.scanLocalDesc')}
        </p>
        <Button
          size="lg"
          onClick={handleScan}
          className="gap-2 px-8"
        >
          <ScanLine className="h-5 w-5" />
          {t('installSkill.scanLocalBtn')}
        </Button>
      </div>
    )
  }

  if (scanning) {
    return (
      <div className="flex flex-col items-center justify-center gap-4 py-16">
        <Loader2 className="h-10 w-10 animate-spin text-muted-foreground" />
        <p className="text-sm text-muted-foreground">{t('installSkill.scanning')}</p>
      </div>
    )
  }

  const isLibrarySelfReference = (s: SkillWithStatus) => {
    const source = s.skill.original_source_path
    const lib = s.skill.library_path
    if (!source) return false
    return source.replace(/\/$/, '') === lib.replace(/\/$/, '')
  }

  const isAlreadyInstalled = (s: SkillWithStatus) =>
    installedIds.has(s.skill.id) || isInLibrary(s) || isLibrarySelfReference(s)

  const pendingSkills = skills?.filter((s) => !isAlreadyInstalled(s)) ?? []
  const doneSkills = skills?.filter((s) => isAlreadyInstalled(s)) ?? []
  const allPendingSelected =
    pendingSkills.length > 0 &&
    pendingSkills.every((s) => selectedIds.has(s.skill.id))

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          {pendingSkills.length > 0 && (
            <label className="flex cursor-pointer items-center gap-2 text-sm text-muted-foreground">
              <Checkbox
                checked={allPendingSelected}
                onCheckedChange={toggleSelectAll}
              />
              <span>{t('installSkill.selectAll')}</span>
            </label>
          )}
          <span className="text-sm text-muted-foreground">
            {t('installSkill.scanSummary', {
              count: pendingSkills.length,
            })}
          </span>
        </div>
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={handleScan}
            disabled={importing}
          >
            <ScanLine className="mr-1 h-4 w-4" />
            {t('installSkill.rescan')}
          </Button>
          {pendingSkills.length > 0 && (
            <Button
              size="sm"
              onClick={handleImportSelected}
              disabled={selectedIds.size === 0 || importing}
            >
              {importing ? (
                <>
                  <Loader2 className="mr-1 h-4 w-4 animate-spin" />
                  {t('installSkill.importing')}
                </>
              ) : (
                t('installSkill.importSelected', { count: selectedIds.size })
              )}
            </Button>
          )}
        </div>
      </div>

      {scanDiff && (
        <div className="flex items-center gap-2">
          {scanDiff.added.length > 0 && (
            <Badge variant="default" className="bg-green-600 hover:bg-green-700">
              +{scanDiff.added.length} new
            </Badge>
          )}
          {scanDiff.updated.length > 0 && (
            <Badge variant="default" className="bg-amber-500 hover:bg-amber-600">
              ~{scanDiff.updated.length} updated
            </Badge>
          )}

        </div>
      )}

      {error && (
        <div className="flex items-center gap-2 rounded-lg bg-red-50 px-4 py-3 text-sm text-red-600">
          <AlertCircle className="h-4 w-4 shrink-0" />
          {error}
        </div>
      )}

      {resultMsg && (
        <div className="flex items-center gap-2 rounded-lg bg-green-50 px-4 py-3 text-sm text-green-600">
          <Check className="h-4 w-4 shrink-0" />
          {resultMsg}
        </div>
      )}

      {skills && skills.length === 0 && (
        <div className="flex flex-col items-center justify-center gap-4 py-12">
          <ScanLine className="h-10 w-10 text-muted-foreground" />
          <p className="text-sm text-muted-foreground">{t('installSkill.noSkillsFound')}</p>
          <Button variant="outline" onClick={handleScan}>
            {t('installSkill.rescan')}
          </Button>
        </div>
      )}

      <div className="space-y-2">
        {pendingSkills.map((sws) => (
          <Card
            key={sws.skill.id}
            className="flex items-start gap-3 p-4"
          >
            <div className="mt-1">
              <Checkbox
                checked={selectedIds.has(sws.skill.id)}
                onCheckedChange={() => toggleSelect(sws.skill.id)}
              />
            </div>
            <div className="min-w-0 flex-1 space-y-1">
              <p className="text-sm font-medium">
                {sws.skill.name}
              </p>
              <p className="text-xs text-muted-foreground">
                {sws.skill.description || t('installSkill.noDescription')}
              </p>
              <p className="text-xs text-muted-foreground">
                {t('installSkill.source')}: {sws.skill.library_path}
                {sws.skill.original_source_path && (
                  <span className="ml-2">
                    {t('installSkill.source2')}: {sws.skill.original_source_path}
                  </span>
                )}
              </p>
            </div>
            <Button
              size="sm"
              onClick={() => handleImportOne(sws)}
              disabled={importing}
            >
              {t('installSkill.import')}
            </Button>
          </Card>
        ))}

        {doneSkills.map((sws) => (
          <Card
            key={sws.skill.id}
            className="flex items-center gap-3 bg-muted/30 p-4"
          >
            <Badge
              variant="secondary"
              className="shrink-0"
            >
              {t('installSkill.imported')}
            </Badge>
            <div className="min-w-0 flex-1 space-y-1">
              <p className="text-sm font-medium text-muted-foreground">
                {sws.skill.name}
              </p>
              <p className="text-xs text-muted-foreground">
                {sws.skill.description || t('installSkill.noDescription')}
              </p>
            </div>
          </Card>
        ))}
      </div>
    </div>
  )
}

export default function InstallSkill() {
  const { t } = useTranslation()

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-2xl font-semibold text-foreground">
          {t('installSkill.title')}
        </h2>
        <p className="mt-1 text-sm text-muted-foreground">
          {t('installSkill.subtitle')}
        </p>
      </div>

      <Tabs defaultValue="local" className="w-full">
        <TabsList className="w-full">
          <TabsTrigger value="local" className="flex-1">
            {t('installSkill.tabLocal')}
          </TabsTrigger>
          <TabsTrigger value="git" className="flex-1">
            {t('installSkill.tabGit')}
          </TabsTrigger>
          <TabsTrigger value="scan" className="flex-1">
            {t('installSkill.tabScan')}
          </TabsTrigger>
        </TabsList>

        <TabsContent value="local" className="mt-4">
          <LocalInstallTab />
        </TabsContent>
        <TabsContent value="git" className="mt-4">
          <GitInstallTab />
        </TabsContent>
        <TabsContent value="scan" className="mt-4">
          <ScanLocalTab />
        </TabsContent>
      </Tabs>
    </div>
  )
}
