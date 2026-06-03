import { useState, useEffect, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { useSync } from '@/hooks/use-sync'
import { invokeCommand } from '@/api'
import { getVersion } from '@tauri-apps/api/app'
import { checkForUpdate } from '@/api/version'
import UpdateDialog from '@/components/UpdateDialog'
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
  const sync = useSync()
  const [archivePassword, setArchivePassword] = useState('')
  const [showAddProvider, setShowAddProvider] = useState(false)

  const [showAddTool, setShowAddTool] = useState(false)
  const [newToolName, setNewToolName] = useState('')
  const [newToolPath, setNewToolPath] = useState('')

  const [editedRepoPath, setEditedRepoPath] = useState<string | null>(null)
  const [version, setVersion] = useState('')
  const [checkingUpdate, setCheckingUpdate] = useState(false)
  const [updateInfo, setUpdateInfo] = useState<{
    hasUpdate: boolean
    currentVersion: string
    latestVersion: string | null
  } | null>(null)
  const [updateError, setUpdateError] = useState<string | null>(null)
  const [showUpdateDialog, setShowUpdateDialog] = useState(false)

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

  useEffect(() => {
    getVersion().then(setVersion).catch(() => setVersion(''))
  }, [])

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


  const handleToggleDebugLogging = async () => {
    if (!config) return
    setSaving(true)
    setError(null)
    try {
      const updated: AppConfigJson = {
        ...config,
        debug_logging: !config.debug_logging,
      }
      await updateConfig(updated)
      setConfig(updated)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setSaving(false)
    }
  }

  const handleCheckUpdate = async () => {
    setCheckingUpdate(true)
    setUpdateError(null)
    setUpdateInfo(null)
    setShowUpdateDialog(false)
    try {
      const result = await checkForUpdate()
      setUpdateInfo(result)
      if (result.hasUpdate && result.latestVersion) {
        setShowUpdateDialog(true)
      }
    } catch (err) {
      setUpdateError(err instanceof Error ? err.message : String(err))
    } finally {
      setCheckingUpdate(false)
    }
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


      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-base">{t('settings.debugLogging')}</CardTitle>
          <p className="mt-0.5 text-xs text-muted-foreground">{t('settings.debugLoggingDesc')}</p>
        </CardHeader>
        <CardContent>
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <button
                type="button"
                role="switch"
                aria-checked={config?.debug_logging ?? false}
                disabled={saving}
                onClick={handleToggleDebugLogging}
                className={`relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 focus:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 ${config?.debug_logging ? 'bg-primary' : 'bg-muted'}`}
              >
                <span className={`pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow ring-0 transition duration-200 ${config?.debug_logging ? 'translate-x-4' : 'translate-x-0'}`} />
              </button>
              <span className="text-sm">
                {config?.debug_logging ? t('settings.debugLoggingOn') : t('settings.debugLoggingOff')}
              </span>
            </div>
          </div>
          <p className="mt-2 text-xs text-muted-foreground">
            {t('settings.debugLoggingHint')}
          </p>
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="flex flex-row items-center justify-between pb-3">
          <div>
            <CardTitle className="text-base">{t('sync.title')}</CardTitle>
            <p className="mt-0.5 text-xs text-muted-foreground">{t('sync.subtitle')}</p>
          </div>
          <Button
            variant="outline"
            size="sm"
            onClick={() => setShowAddProvider(true)}
            data-testid="sync-add-provider-btn"
          >
            <Plus className="mr-1.5 h-3.5 w-3.5" />
            {t('sync.addProvider')}
          </Button>
        </CardHeader>
        <CardContent>
          {/* Archive password (encrypted via SENSITIVE_KEYS) */}
          <div className="mb-4 rounded-lg border p-3">
            <label className="text-sm font-medium">
              {t('sync.archivePassword')}
            </label>
            <p className="mt-0.5 text-xs text-muted-foreground">
              {t('sync.archivePasswordHint')}
            </p>
            <div className="mt-2 flex gap-2">
              <Input
                type="password"
                value={archivePassword}
                onChange={(e) => setArchivePassword(e.target.value)}
                placeholder="••••••"
                className="flex-1"
                data-testid="sync-archive-password-input"
              />
              <Button
                variant="outline"
                size="sm"
                disabled={!archivePassword.trim()}
                onClick={async () => {
                  try {
                    await invokeCommand('set_config_value', {
                      key: 'backup_archive_password',
                      value: archivePassword,
                    });
                  } catch (e) {
                    console.error(e);
                  }
                }}
              >
                {t('sync.setPassword')}
              </Button>
            </div>
          </div>

          {sync.error && (
            <p className="mb-2 text-xs text-destructive">{sync.error.message}</p>
          )}

          {sync.providers.length === 0 ? (
            <p className="py-4 text-center text-sm text-muted-foreground">
              {t('sync.noProviders')}
            </p>
          ) : (
            <div className="space-y-2">
              {sync.providers.map((p) => (
                <div
                  key={p.id}
                  className="flex items-center justify-between gap-3 rounded-lg border p-3"
                  data-testid={`sync-provider-row-${p.id}`}
                >
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-medium">{p.name}</span>
                      <span className="rounded bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">
                        {p.kind}
                      </span>
                    </div>
                    <p className="mt-0.5 text-xs text-muted-foreground">
                      {t('sync.lastSync')}: {p.last_sync_at ?? t('sync.neverSynced')}
                    </p>
                  </div>
                  <div className="flex shrink-0 items-center gap-1">
                    <Button
                      variant="outline"
                      size="sm"
                      disabled={sync.loading}
                      onClick={() => sync.testConnection(p.id).catch(() => {})}
                    >
                      {t('sync.testConnection')}
                    </Button>
                    <Button
                      size="sm"
                      disabled={sync.loading}
                      onClick={() => sync.syncUp(p.id).catch(() => {})}
                      data-testid={`sync-up-btn-${p.id}`}
                    >
                      {t('sync.syncUp')}
                    </Button>
                    <button
                      type="button"
                      aria-label={t('sync.deleteProvider')}
                      onClick={() => {
                        if (window.confirm(t('sync.confirmDelete'))) {
                          void sync.remove(p.id);
                        }
                      }}
                      className="flex h-7 w-7 items-center justify-center rounded text-muted-foreground hover:text-destructive"
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}

          {/* Recent history — last 5 across all providers */}
          {sync.history.length > 0 && (
            <div className="mt-4 border-t pt-3">
              <p className="mb-2 text-xs font-medium text-muted-foreground">
                {t('sync.history')}
              </p>
              <div className="space-y-1">
                {sync.history.slice(0, 5).map((h) => (
                  <div
                    key={h.id}
                    className="flex items-center justify-between text-xs"
                  >
                    <span className="text-muted-foreground">
                      {h.started_at} · {h.direction} · {h.provider_id.slice(0, 8)}
                    </span>
                    <span
                      className={
                        h.status === 'success'
                          ? 'text-green-600'
                          : h.status === 'cancelled'
                            ? 'text-muted-foreground'
                            : 'text-destructive'
                      }
                    >
                      {h.status}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-base">{t('settings.updateCheck')}</CardTitle>
          <p className="mt-0.5 text-xs text-muted-foreground">{t('settings.updateCheckDesc')}</p>
        </CardHeader>
        <CardContent>
          <div className="flex items-center gap-3">
            <Button
              variant="outline"
              size="sm"
              onClick={handleCheckUpdate}
              disabled={checkingUpdate}
            >
              {checkingUpdate ? (
                <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
              ) : null}
              {t('settings.checkUpdate')}
            </Button>
            {updateInfo && (
              <span className="text-sm">
                {updateInfo.hasUpdate ? (
                  <span className="text-green-600">
                    {t('settings.updateAvailable', {
                      current: updateInfo.currentVersion,
                      latest: updateInfo.latestVersion,
                    })}
                  </span>
                ) : (
                  <span className="text-muted-foreground">
                    {t('settings.upToDate', { version: updateInfo.currentVersion })}
                  </span>
                )}
              </span>
            )}
          </div>
          {updateError && (
            <p className="mt-2 text-xs text-red-600">{updateError}</p>
          )}
        </CardContent>
      </Card>

      <div className="mt-auto border-t border-border pt-6 pb-2">
        <div className="flex items-center gap-3 text-sm text-muted-foreground">
          <img src={appIconUrl} alt="Skills Panel" className="h-5 w-5" />
          <span className="font-medium">{t('settings.footer')}</span>
          <span className="text-xs">{version}</span>
        </div>
        <p className="mt-1 text-xs text-muted-foreground/60">@zhangsubo.cn</p>
      </div>

      {updateInfo?.latestVersion && (
        <UpdateDialog
          open={showUpdateDialog}
          onClose={() => setShowUpdateDialog(false)}
          currentVersion={updateInfo.currentVersion}
          latestVersion={updateInfo.latestVersion}
        />
      )}

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

      <Dialog open={showAddProvider} onOpenChange={(open) => !open && setShowAddProvider(false)}>
        <AddSyncProviderDialog
          onClose={() => setShowAddProvider(false)}
          onCreate={async (name, kind, configJson) => {
            try {
              await sync.create(name, kind, configJson);
              setShowAddProvider(false);
            } catch (e) {
              console.error(e);
            }
          }}
        />
      </Dialog>
    </div>
  )
}

/**
 * Minimal "add provider" dialog. Renders kind-specific fields inline
 * (GitHub: repo+branch; WebDAV: url+username+password+remote_path).
 * Config is JSON-encoded before being passed to the backend.
 */
function AddSyncProviderDialog({
  onClose,
  onCreate,
}: {
  onClose: () => void;
  onCreate: (name: string, kind: 'github_zip' | 'webdav', configJson: string) => Promise<void>;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState('');
  const [kind, setKind] = useState<'github_zip' | 'webdav'>('webdav');
  const [repo, setRepo] = useState('');
  const [branch, setBranch] = useState('main');
  const [url, setUrl] = useState('');
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [remotePath, setRemotePath] = useState('backups');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      // Credentials go to SENSITIVE_KEYS (encrypted in DB) — config_json
      // holds only the non-secret metadata. This keeps the on-disk record
      // clean and ensures `build_sync_provider` always reads fresh creds
      // from the encrypted store rather than from a stale config_json blob.
      if (kind === 'webdav') {
        if (username) {
          await invokeCommand('set_config_value', {
            key: 'webdav_username',
            value: username,
          });
        }
        if (password) {
          await invokeCommand('set_config_value', {
            key: 'webdav_password',
            value: password,
          });
        }
      }
      const configJson =
        kind === 'github_zip'
          ? JSON.stringify({ repo, branch })
          : JSON.stringify({ url, remote_path: remotePath });
      await onCreate(name.trim(), kind, configJson);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <DialogContent>
      <DialogHeader>
        <DialogTitle>{t('sync.addProvider')}</DialogTitle>
        <DialogDescription>{t('sync.subtitle')}</DialogDescription>
      </DialogHeader>
      <div className="space-y-3">
        <div>
          <label className="text-sm text-muted-foreground">Name</label>
          <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="personal-nextcloud" />
        </div>
        <div>
          <label className="text-sm text-muted-foreground">Kind</label>
          <div className="mt-1 flex gap-2">
            <Button
              type="button"
              variant={kind === 'webdav' ? 'default' : 'outline'}
              size="sm"
              onClick={() => setKind('webdav')}
            >
              {t('sync.kind.webdav')}
            </Button>
            <Button
              type="button"
              variant={kind === 'github_zip' ? 'default' : 'outline'}
              size="sm"
              onClick={() => setKind('github_zip')}
            >
              {t('sync.kind.githubZip')}
            </Button>
          </div>
        </div>
        {kind === 'github_zip' ? (
          <>
            <div>
              <label className="text-sm text-muted-foreground">{t('sync.fields.repo')}</label>
              <Input
                value={repo}
                onChange={(e) => setRepo(e.target.value)}
                placeholder={t('sync.fields.repoPlaceholder')}
              />
            </div>
            <div>
              <label className="text-sm text-muted-foreground">{t('sync.fields.branch')}</label>
              <Input
                value={branch}
                onChange={(e) => setBranch(e.target.value)}
                placeholder={t('sync.fields.branchPlaceholder')}
              />
            </div>
          </>
        ) : (
          <>
            <div>
              <label className="text-sm text-muted-foreground">{t('sync.fields.url')}</label>
              <Input
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                placeholder={t('sync.fields.urlPlaceholder')}
              />
            </div>
            <div className="grid grid-cols-2 gap-2">
              <div>
                <label className="text-sm text-muted-foreground">{t('sync.fields.username')}</label>
                <Input
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  autoComplete="username"
                />
              </div>
              <div>
                <label className="text-sm text-muted-foreground">{t('sync.fields.password')}</label>
                <Input
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  autoComplete="current-password"
                />
              </div>
            </div>
            <div>
              <label className="text-sm text-muted-foreground">{t('sync.fields.remotePath')}</label>
              <Input
                value={remotePath}
                onChange={(e) => setRemotePath(e.target.value)}
                placeholder={t('sync.fields.remotePathPlaceholder')}
              />
            </div>
          </>
        )}
        {error && <p className="text-xs text-destructive">{error}</p>}
      </div>
      <DialogFooter>
        <Button variant="outline" onClick={onClose}>
          {t('library.cancel')}
        </Button>
        <Button
          onClick={submit}
          disabled={busy || !name.trim() || (kind === 'github_zip' ? !repo.trim() : !url.trim())}
        >
          {t('sync.addProvider')}
        </Button>
      </DialogFooter>
    </DialogContent>
  );
}
