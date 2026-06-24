import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Loader2, Plus } from 'lucide-react'
import { syncAddProvider } from '@/api/sync'

type ProviderKind = 'webdav' | 's3' | 'sftp'

interface AddProviderDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  onAdded: () => void
}

interface WebdavConfig {
  url: string
  username: string
  password: string
}

interface S3Config {
  endpoint: string
  bucket: string
  access_key_id: string
  secret_access_key: string
  region: string
}

interface SftpConfig {
  host: string
  port: string
  username: string
  password: string
  key_file: string
}

const KINDS: { value: ProviderKind; labelKey: string }[] = [
  { value: 'webdav', labelKey: 'sync.kind.webdav' },
  { value: 's3', labelKey: 'sync.kind.s3' },
  { value: 'sftp', labelKey: 'sync.kind.sftp' },
]

export default function AddProviderDialog({
  open,
  onOpenChange,
  onAdded,
}: AddProviderDialogProps) {
  const { t } = useTranslation()
  const [kind, setKind] = useState<ProviderKind>('webdav')
  const [name, setName] = useState('')
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // WebDAV fields
  const [webdavUrl, setWebdavUrl] = useState('')
  const [webdavUser, setWebdavUser] = useState('')
  const [webdavPass, setWebdavPass] = useState('')
  const [showWebdavPass, setShowWebdavPass] = useState(false)

  // S3 fields
  const [s3Endpoint, setS3Endpoint] = useState('')
  const [s3Bucket, setS3Bucket] = useState('')
  const [s3AccessKey, setS3AccessKey] = useState('')
  const [s3SecretKey, setS3SecretKey] = useState('')
  const [s3Region, setS3Region] = useState('')
  const [showS3Secret, setShowS3Secret] = useState(false)

  // SFTP fields
  const [sftpHost, setSftpHost] = useState('')
  const [sftpPort, setSftpPort] = useState('22')
  const [sftpUser, setSftpUser] = useState('')
  const [sftpPass, setSftpPass] = useState('')
  const [sftpKeyFile, setSftpKeyFile] = useState('')
  const [showSftpPass, setShowSftpPass] = useState(false)

  const resetForm = () => {
    setKind('webdav')
    setName('')
    setError(null)
    setWebdavUrl('')
    setWebdavUser('')
    setWebdavPass('')
    setShowWebdavPass(false)
    setS3Endpoint('')
    setS3Bucket('')
    setS3AccessKey('')
    setS3SecretKey('')
    setS3Region('')
    setShowS3Secret(false)
    setSftpHost('')
    setSftpPort('22')
    setSftpUser('')
    setSftpPass('')
    setSftpKeyFile('')
    setShowSftpPass(false)
  }

  const buildConfigJson = (): string => {
    switch (kind) {
      case 'webdav': {
        const cfg: WebdavConfig = {
          url: webdavUrl,
          username: webdavUser,
          password: webdavPass,
        }
        return JSON.stringify(cfg)
      }
      case 's3': {
        const cfg: S3Config = {
          endpoint: s3Endpoint,
          bucket: s3Bucket,
          access_key_id: s3AccessKey,
          secret_access_key: s3SecretKey,
          region: s3Region,
        }
        return JSON.stringify(cfg)
      }
      case 'sftp': {
        const cfg: SftpConfig = {
          host: sftpHost,
          port: sftpPort,
          username: sftpUser,
          password: sftpPass,
          key_file: sftpKeyFile,
        }
        return JSON.stringify(cfg)
      }
    }
  }

  const isFormValid = (): boolean => {
    if (!name.trim()) return false
    switch (kind) {
      case 'webdav':
        return webdavUrl.trim().length > 0
      case 's3':
        return s3Endpoint.trim().length > 0 && s3Bucket.trim().length > 0
      case 'sftp':
        return sftpHost.trim().length > 0
    }
  }

  const handleSubmit = async () => {
    if (!isFormValid()) return
    setSaving(true)
    setError(null)
    try {
      const id = name.trim().toLowerCase().replace(/\s+/g, '-')
      await syncAddProvider(id, name.trim(), kind, buildConfigJson())
      resetForm()
      onOpenChange(false)
      onAdded()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setSaving(false)
    }
  }

  const handleClose = () => {
    if (saving) return
    resetForm()
    onOpenChange(false)
  }

  return (
    <Dialog open={open} onOpenChange={handleClose}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('sync.addProvider')}</DialogTitle>
          <DialogDescription>
            {t('sync.subtitle')}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          {/* Kind selector */}
          <div className="space-y-2">
            <label className="text-sm text-muted-foreground">Type</label>
            <div className="flex gap-2">
              {KINDS.map((k) => (
                <button
                  key={k.value}
                  type="button"
                  onClick={() => setKind(k.value)}
                  className={`rounded-md border px-3 py-1.5 text-sm transition-colors ${
                    kind === k.value
                      ? 'border-primary bg-primary text-primary-foreground'
                      : 'border-border bg-background text-foreground hover:bg-muted'
                  }`}
                >
                  {t(k.labelKey)}
                </button>
              ))}
            </div>
          </div>

          {/* Name */}
          <div className="space-y-2">
            <label className="text-sm text-muted-foreground">Name</label>
            <Input
              placeholder="My Backup"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </div>

          {/* WebDAV fields */}
          {kind === 'webdav' && (
            <>
              <div className="space-y-2">
                <label className="text-sm text-muted-foreground">{t('sync.fields.url')}</label>
                <Input
                  placeholder={t('sync.fields.urlPlaceholder')}
                  value={webdavUrl}
                  onChange={(e) => setWebdavUrl(e.target.value)}
                />
              </div>
              <div className="space-y-2">
                <label className="text-sm text-muted-foreground">{t('sync.fields.username')}</label>
                <Input
                  placeholder={t('sync.fields.username')}
                  value={webdavUser}
                  onChange={(e) => setWebdavUser(e.target.value)}
                />
              </div>
              <div className="space-y-2">
                <label className="text-sm text-muted-foreground">{t('sync.fields.password')}</label>
                <div className="relative">
                  <Input
                    type={showWebdavPass ? 'text' : 'password'}
                    placeholder={t('sync.fields.password')}
                    value={webdavPass}
                    onChange={(e) => setWebdavPass(e.target.value)}
                    className="pr-16"
                  />
                  <button
                    type="button"
                    onClick={() => setShowWebdavPass(!showWebdavPass)}
                    className="absolute right-2 top-1/2 -translate-y-1/2 text-xs text-muted-foreground hover:text-foreground"
                  >
                    {showWebdavPass ? 'Hide' : 'Show'}
                  </button>
                </div>
              </div>
            </>
          )}

          {/* S3 fields */}
          {kind === 's3' && (
            <>
              <div className="space-y-2">
                <label className="text-sm text-muted-foreground">Endpoint</label>
                <Input
                  placeholder="https://s3.amazonaws.com"
                  value={s3Endpoint}
                  onChange={(e) => setS3Endpoint(e.target.value)}
                />
              </div>
              <div className="space-y-2">
                <label className="text-sm text-muted-foreground">Bucket</label>
                <Input
                  placeholder="my-bucket"
                  value={s3Bucket}
                  onChange={(e) => setS3Bucket(e.target.value)}
                />
              </div>
              <div className="space-y-2">
                <label className="text-sm text-muted-foreground">Access Key ID</label>
                <Input
                  placeholder="AKIAIOSFODNN7EXAMPLE"
                  value={s3AccessKey}
                  onChange={(e) => setS3AccessKey(e.target.value)}
                />
              </div>
              <div className="space-y-2">
                <label className="text-sm text-muted-foreground">Secret Access Key</label>
                <div className="relative">
                  <Input
                    type={showS3Secret ? 'text' : 'password'}
                    placeholder="wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
                    value={s3SecretKey}
                    onChange={(e) => setS3SecretKey(e.target.value)}
                    className="pr-16"
                  />
                  <button
                    type="button"
                    onClick={() => setShowS3Secret(!showS3Secret)}
                    className="absolute right-2 top-1/2 -translate-y-1/2 text-xs text-muted-foreground hover:text-foreground"
                  >
                    {showS3Secret ? 'Hide' : 'Show'}
                  </button>
                </div>
              </div>
              <div className="space-y-2">
                <label className="text-sm text-muted-foreground">Region</label>
                <Input
                  placeholder="us-east-1"
                  value={s3Region}
                  onChange={(e) => setS3Region(e.target.value)}
                />
              </div>
            </>
          )}

          {/* SFTP fields */}
          {kind === 'sftp' && (
            <>
              <div className="space-y-2">
                <label className="text-sm text-muted-foreground">Host</label>
                <Input
                  placeholder="sftp.example.com"
                  value={sftpHost}
                  onChange={(e) => setSftpHost(e.target.value)}
                />
              </div>
              <div className="space-y-2">
                <label className="text-sm text-muted-foreground">Port</label>
                <Input
                  placeholder="22"
                  value={sftpPort}
                  onChange={(e) => setSftpPort(e.target.value)}
                />
              </div>
              <div className="space-y-2">
                <label className="text-sm text-muted-foreground">{t('sync.fields.username')}</label>
                <Input
                  placeholder={t('sync.fields.username')}
                  value={sftpUser}
                  onChange={(e) => setSftpUser(e.target.value)}
                />
              </div>
              <div className="space-y-2">
                <label className="text-sm text-muted-foreground">{t('sync.fields.password')}</label>
                <div className="relative">
                  <Input
                    type={showSftpPass ? 'text' : 'password'}
                    placeholder={t('sync.fields.password')}
                    value={sftpPass}
                    onChange={(e) => setSftpPass(e.target.value)}
                    className="pr-16"
                  />
                  <button
                    type="button"
                    onClick={() => setShowSftpPass(!showSftpPass)}
                    className="absolute right-2 top-1/2 -translate-y-1/2 text-xs text-muted-foreground hover:text-foreground"
                  >
                    {showSftpPass ? 'Hide' : 'Show'}
                  </button>
                </div>
              </div>
              <div className="space-y-2">
                <label className="text-sm text-muted-foreground">Key File Path</label>
                <Input
                  placeholder="/home/user/.ssh/id_rsa"
                  value={sftpKeyFile}
                  onChange={(e) => setSftpKeyFile(e.target.value)}
                />
              </div>
            </>
          )}

          {error && (
            <p className="text-sm text-red-600">{error}</p>
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={handleClose} disabled={saving}>
            {t('library.cancel')}
          </Button>
          <Button onClick={handleSubmit} disabled={!isFormValid() || saving}>
            {saving ? (
              <Loader2 className="mr-1.5 h-4 w-4 animate-spin" />
            ) : (
              <Plus className="mr-1.5 h-4 w-4" />
            )}
            {t('sync.addProvider')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}