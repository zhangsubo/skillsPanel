import { useState, useEffect, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import {
  getConfig,
  updateConfig,
  type AppConfigJson,
  type ToolJson,
} from '@/api/settings'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import { Skeleton } from '@/components/ui/skeleton'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog'
import {
  Wrench,
  Plus,
  Trash2,
  Folder,
  FolderOpen,
  Loader2,
  AlertCircle,
  Save,
} from 'lucide-react'

const isTauriEnv = (): boolean =>
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

const appIconUrl = new URL('../../src-tauri/icons/32x32.png', import.meta.url).href

function toolDisplayName(id: string): string {
  return id
    .replace(/[-_]/g, ' ')
    .split(' ')
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
    .join(' ')
}

export default function Settings() {
  const { t } = useTranslation()
  const [config, setConfig] = useState<AppConfigJson | null>(null)
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const [showAddTool, setShowAddTool] = useState(false)
  const [newToolName, setNewToolName] = useState('')
  const [newToolPath, setNewToolPath] = useState('')

  const [editedRepoPath, setEditedRepoPath] = useState<string | null>(null)

  const loadConfig = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const cfg = await getConfig()
      setConfig(cfg)
      setEditedRepoPath(cfg.library_path)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    loadConfig()
  }, [loadConfig])

  const tools: ToolJson[] = config?.tools ?? []
  const repoPath = editedRepoPath ?? config?.library_path ?? ''
  const repoPathChanged = editedRepoPath !== null && config !== null && editedRepoPath !== config.library_path

  const handleToggleTool = async (toolId: string, enabled: boolean) => {
    if (!config) return
    setSaving(true)
    setError(null)
    try {
      const updated: AppConfigJson = {
        ...config,
        tools: config.tools.map((t) =>
          t.id === toolId ? { ...t, enabled } : t,
        ),
      }
      await updateConfig(updated)
      setConfig(updated)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setSaving(false)
    }
  }

  const handleDeleteTool = async (toolId: string) => {
    if (!config) return
    setSaving(true)
    setError(null)
    try {
      const updated: AppConfigJson = {
        ...config,
        tools: config.tools.filter((t) => t.id !== toolId),
      }
      await updateConfig(updated)
      setConfig(updated)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setSaving(false)
    }
  }

  const handleAddTool = async () => {
    if (!config || !newToolName.trim() || !newToolPath.trim()) return
    setSaving(true)
    setError(null)
    try {
      const id = newToolName.trim().toLowerCase().replace(/\s+/g, '-')
      if (config.tools.some((t) => t.id === id)) {
        throw new Error(`Tool '${newToolName.trim()}' already exists`)
      }
      const updated: AppConfigJson = {
        ...config,
        tools: [
          ...config.tools,
          {
            id,
            name: newToolName.trim(),
            path: newToolPath.trim(),
            enabled: true,
            is_custom: true,
          },
        ],
      }
      await updateConfig(updated)
      setConfig(updated)
      setShowAddTool(false)
      setNewToolName('')
      setNewToolPath('')
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setSaving(false)
    }
  }

  const handlePickFolder = async (setter: (v: string) => void) => {
    if (!isTauriEnv()) return
    try {
      const { open } = await import('@tauri-apps/plugin-dialog')
      const selected = await open({ directory: true, multiple: false })
      if (selected && typeof selected === 'string') {
        setter(selected)
      }
    } catch {}
  }

  const handleChangeRepoDir = async () => {
    if (!isTauriEnv()) return
    await handlePickFolder(setEditedRepoPath)
  }

  const handleSaveRepoPath = async () => {
    if (!config || !editedRepoPath) return
    setSaving(true)
    setError(null)
    try {
      const updated: AppConfigJson = { ...config, library_path: editedRepoPath }
      await updateConfig(updated)
      setConfig(updated)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setSaving(false)
    }
  }

  const handleOpenRepoDir = async () => {
    if (!isTauriEnv()) return
    try {
      const { open } = await import('@tauri-apps/plugin-shell')
      await open(repoPath)
    } catch {}
  }

  if (loading) {
    return (
      <div className="flex h-full flex-col gap-6 p-6">
        <Skeleton className="h-8 w-32" />
        <Skeleton className="h-40 w-full" />
        <Skeleton className="h-24 w-full" />
        <Skeleton className="h-16 w-full" />
      </div>
    )
  }

  if (error && !config) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4 p-6">
        <AlertCircle className="h-8 w-8 text-destructive" />
        <p className="text-sm text-destructive">{error}</p>
        <Button variant="outline" onClick={loadConfig}>
          {t('error.retry')}
        </Button>
      </div>
    )
  }

  return (
    <div className="flex h-full flex-col gap-6 overflow-auto p-6">
      <h2 className="text-2xl font-semibold text-foreground">{t('settings.title')}</h2>

      {error && (
        <div className="flex items-center gap-2 rounded-lg bg-red-50 px-4 py-3 text-sm text-red-600">
          <AlertCircle className="h-4 w-4 shrink-0" />
          {error}
        </div>
      )}

      <Card>
        <CardHeader className="flex flex-row items-center justify-between pb-3">
          <div>
            <CardTitle className="text-base">{t('settings.tools')}</CardTitle>
            <p className="mt-0.5 text-xs text-muted-foreground">{t('settings.toolsDesc')}</p>
          </div>
          <Badge variant="secondary">{tools.length}</Badge>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
            {tools.map((tool) => (
              <div
                key={tool.id}
                className="group relative rounded-lg border p-3 transition-colors hover:bg-muted/50"
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <Wrench className="h-4 w-4 shrink-0 text-muted-foreground" />
                      <span className="text-sm font-medium">
                        {tool.name || toolDisplayName(tool.id)}
                      </span>
                    </div>
                    <p className="mt-1 break-all text-xs text-muted-foreground">
                      {tool.path || t('settings.noPath')}
                    </p>
                  </div>
                  <div className="flex shrink-0 items-center gap-1.5">
                    {tool.enabled ? (
                      <Badge variant="default" className="bg-green-600 hover:bg-green-700 text-xs">
                        {t('settings.enabled')}
                      </Badge>
                    ) : (
                      <Badge variant="secondary" className="text-xs">
                        {t('settings.disabled')}
                      </Badge>
                    )}
                    <div className="opacity-0 transition-opacity group-hover:opacity-100">
                      <button
                        type="button"
                        onClick={() => handleDeleteTool(tool.id)}
                        className="flex h-6 w-6 items-center justify-center rounded text-muted-foreground hover:text-destructive"
                        title={t('settings.deleteTool')}
                      >
                        <Trash2 className="h-3.5 w-3.5" />
                      </button>
                    </div>
                  </div>
                </div>
                <div className="mt-2">
                  <button
                    type="button"
                    role="switch"
                    aria-checked={tool.enabled}
                    disabled={saving}
                    onClick={() => handleToggleTool(tool.id, !tool.enabled)}
                    className={`relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 focus:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 ${
                      tool.enabled ? 'bg-primary' : 'bg-muted'
                    }`}
                  >
                    <span
                      className={`pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow ring-0 transition duration-200 ${
                        tool.enabled ? 'translate-x-4' : 'translate-x-0'
                      }`}
                    />
                  </button>
                </div>
              </div>
            ))}

            <button
              type="button"
              onClick={() => setShowAddTool(true)}
              className="flex cursor-pointer items-center justify-center gap-2 rounded-lg border-2 border-dashed border-border p-4 text-sm text-muted-foreground transition-colors hover:border-primary/50 hover:text-primary"
            >
              <Plus className="h-4 w-4" />
              {t('settings.addTool')}
            </button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-base">{t('settings.repository')}</CardTitle>
          <p className="mt-0.5 text-xs text-muted-foreground">{t('settings.repositoryDesc')}</p>
        </CardHeader>
        <CardContent>
          <div className="flex items-center gap-3 rounded-lg bg-muted px-4 py-3">
            <Folder className="h-5 w-5 shrink-0 text-muted-foreground" />
            <span className="flex-1 break-all text-sm font-mono">{repoPath}</span>
          </div>
          <div className="mt-3 flex items-center gap-2 text-xs text-muted-foreground">
            <FolderOpen className="h-3.5 w-3.5" />
            {t('settings.repositoryDefault')}
          </div>
          <div className="mt-3 flex flex-wrap gap-2">
            <Button variant="outline" size="sm" onClick={handleChangeRepoDir}>
              <FolderOpen className="mr-1.5 h-3.5 w-3.5" />
              {t('settings.changeDir')}
            </Button>
            {repoPathChanged && (
              <Button size="sm" onClick={handleSaveRepoPath} disabled={saving}>
                {saving ? (
                  <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
                ) : (
                  <Save className="mr-1.5 h-3.5 w-3.5" />
                )}
                {t('settings.save')}
              </Button>
            )}
            <Button variant="outline" size="sm" onClick={handleOpenRepoDir}>
              <Folder className="mr-1.5 h-3.5 w-3.5" />
              {t('settings.openDir')}
            </Button>
          </div>
        </CardContent>
      </Card>

      <div className="mt-auto border-t border-border pt-6 pb-2">
        <div className="flex items-center gap-3 text-sm text-muted-foreground">
          <img src={appIconUrl} alt="Skills Panel" className="h-5 w-5" />
          <span className="font-medium">{t('settings.footer')}</span>
          <span className="text-xs">0.2.0</span>
        </div>
        <p className="mt-1 text-xs text-muted-foreground/60">@zhangsubo.cn</p>
      </div>

      <Dialog open={showAddTool} onOpenChange={(open) => !open && setShowAddTool(false)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t('settings.addToolTitle')}</DialogTitle>
            <DialogDescription>
              {t('settings.toolsDesc')}
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4">
            <div className="space-y-2">
              <label className="text-sm text-muted-foreground">{t('settings.toolName')}</label>
              <Input
                placeholder="My Custom Tool"
                value={newToolName}
                onChange={(e) => setNewToolName(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <label className="text-sm text-muted-foreground">{t('settings.toolPath')}</label>
              <div className="flex gap-2">
                <Input
                  placeholder="/path/to/skills"
                  value={newToolPath}
                  onChange={(e) => setNewToolPath(e.target.value)}
                  className="flex-1"
                />
                <Button
                  variant="outline"
                  size="icon"
                  onClick={() => handlePickFolder(setNewToolPath)}
                  title={t('settings.changeDir')}
                >
                  <FolderOpen className="h-4 w-4" />
                </Button>
              </div>
            </div>
            <div className="space-y-2">
              <label className="text-sm text-muted-foreground">
                {t('settings.toolWorkspacePath')}
              </label>
              <Input
                placeholder=".my-agent/skills"
              />
              <p className="text-xs text-muted-foreground">{t('settings.toolWorkspaceHint')}</p>
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setShowAddTool(false)}>
              {t('library.cancel')}
            </Button>
            <Button
              onClick={handleAddTool}
              disabled={!newToolName.trim() || !newToolPath.trim() || saving}
            >
              {saving ? (
                <Loader2 className="mr-1.5 h-4 w-4 animate-spin" />
              ) : (
                <Plus className="mr-1.5 h-4 w-4" />
              )}
              {t('settings.addTool')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
