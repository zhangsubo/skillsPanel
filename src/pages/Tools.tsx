import { useState, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { useTools } from '@/hooks/use-tools'
import { useLibrary } from '@/hooks/use-library'
import { cleanStaleLinks } from '@/api/sync'
import { RefreshCw, Trash2, Link2, Unlink2 } from 'lucide-react'
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  CardFooter,
} from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import {
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
} from '@/components/ui/table'

export default function Tools() {
  const { t } = useTranslation()
  const { tools, loading, error, refresh, link, unlink, sync } = useTools()
  const { scanResult, scan } = useLibrary()
  const [syncing, setSyncing] = useState(false)
  const [cleaning, setCleaning] = useState(false)
  const [operating, setOperating] = useState<string | null>(null)
  const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null)
  const [linkInput, setLinkInput] = useState<Record<string, string>>({})

  const linkedSkillsByTool = (() => {
    const map: Record<string, string[]> = {}
    if (scanResult?.skills) {
      for (const sws of scanResult.skills) {
        for (const [toolName, status] of Object.entries(sws.tool_statuses)) {
          if (status === 'linked') {
            if (!map[toolName]) map[toolName] = []
            map[toolName].push(sws.skill.name)
          }
        }
      }
    }
    return map
  })()

  const handleSync = useCallback(async () => {
    setSyncing(true)
    setMessage(null)
    try {
      const count = await sync()
      setMessage({ type: 'success', text: t('tools.synced', { count }) })
    } catch (err) {
      setMessage({ type: 'error', text: err instanceof Error ? err.message : t('tools.syncFailed') })
    } finally {
      setSyncing(false)
    }
  }, [sync, t])

  const handleClean = useCallback(async () => {
    setCleaning(true)
    setMessage(null)
    try {
      const count = await cleanStaleLinks()
      setMessage({ type: 'success', text: t('tools.cleaned', { count }) })
      await refresh()
      await scan()
    } catch (err) {
      setMessage({ type: 'error', text: err instanceof Error ? err.message : t('tools.cleanFailed') })
    } finally {
      setCleaning(false)
    }
  }, [refresh, scan, t])

  const handleLink = useCallback(
    async (skillName: string, toolId: string) => {
      const key = `${toolId}-link`
      setOperating(key)
      setMessage(null)
      try {
        await link(skillName, toolId)
        await scan()
        setLinkInput((prev) => ({ ...prev, [toolId]: '' }))
        setMessage({ type: 'success', text: t('tools.linked', { name: skillName }) })
      } catch (err) {
        setMessage({ type: 'error', text: err instanceof Error ? err.message : t('tools.linkFailed') })
      } finally {
        setOperating(null)
      }
    },
    [link, scan, t],
  )

  const handleUnlink = useCallback(
    async (skillName: string, toolId: string) => {
      const key = `${toolId}-unlink-${skillName}`
      setOperating(key)
      setMessage(null)
      try {
        await unlink(skillName, toolId)
        await scan()
        setMessage({ type: 'success', text: t('tools.unlinked', { name: skillName }) })
      } catch (err) {
        setMessage({ type: 'error', text: err instanceof Error ? err.message : t('tools.unlinkFailed') })
      } finally {
        setOperating(null)
      }
    },
    [unlink, scan, t],
  )

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20">
        <RefreshCw className="h-6 w-6 animate-spin text-muted-foreground" />
        <span className="ml-2 text-muted-foreground">{t('tools.loading')}</span>
      </div>
    )
  }

  if (error) {
    return (
      <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-4">
        <p className="text-sm font-medium text-destructive">{t('tools.loadFailed')}</p>
        <p className="mt-1 text-sm text-destructive/80">{error.message}</p>
        <Button variant="outline" size="sm" className="mt-3" onClick={refresh}>
          {t('error.retry')}
        </Button>
      </div>
    )
  }

  return (
    <div className="space-y-6">
      <div className="flex items-start justify-between">
        <div>
          <h2 className="text-2xl font-semibold text-gray-900 dark:text-gray-100">{t('tools.title')}</h2>
          <p className="mt-1 text-sm text-gray-600 dark:text-gray-400">
            {t('tools.subtitle')}
          </p>
        </div>
        <div className="flex gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={handleSync}
            disabled={syncing}
          >
            <RefreshCw className={`mr-1.5 h-3.5 w-3.5 ${syncing ? 'animate-spin' : ''}`} />
            {syncing ? t('tools.syncing') : t('tools.syncSkills')}
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={handleClean}
            disabled={cleaning}
          >
            <Trash2 className="mr-1.5 h-3.5 w-3.5" />
            {cleaning ? t('tools.cleaning') : t('tools.cleanLinks')}
          </Button>
        </div>
      </div>

      {message && (
        <div
          className={`rounded-md border px-4 py-3 text-sm ${
            message.type === 'success'
              ? 'border-green-200 bg-green-50 text-green-800 dark:border-green-800 dark:bg-green-900/20 dark:text-green-300'
              : 'border-red-200 bg-red-50 text-red-800 dark:border-red-800 dark:bg-red-900/20 dark:text-red-300'
          }`}
        >
          {message.text}
        </div>
      )}

      {tools.length === 0 ? (
        <div className="rounded-lg border border-dashed border-gray-300 py-12 text-center dark:border-gray-700">
          <p className="text-sm text-gray-500 dark:text-gray-400">{t('tools.noTools')}</p>
          <p className="mt-1 text-xs text-gray-400 dark:text-gray-500">
            {t('tools.noToolsHint')}
          </p>
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-4 lg:grid-cols-2 xl:grid-cols-3">
          {tools.map((tool) => {
            const linkedSkills = linkedSkillsByTool[tool.name] ?? []
            const inputValue = linkInput[tool.id] ?? ''

            return (
              <Card key={tool.id}>
                <CardHeader className="pb-3">
                  <div className="flex items-start justify-between">
                    <CardTitle className="text-base">{tool.name}</CardTitle>
                    <Badge variant={tool.enabled ? 'default' : 'secondary'}>
                      {tool.enabled ? t('tools.enabled') : t('tools.disabled')}
                    </Badge>
                  </div>
                  <CardDescription className="truncate text-xs" title={tool.path}>
                    {tool.path}
                  </CardDescription>
                </CardHeader>

                <CardContent className="space-y-3">
                  <div className="flex items-center gap-1.5 text-sm">
                    <Link2 className="h-3.5 w-3.5 text-muted-foreground" />
                    <span className="text-muted-foreground">
                      {t('tools.linkedSkills', { count: linkedSkills.length })}
                    </span>
                  </div>

                  {linkedSkills.length > 0 && (
                    <Table>
                      <TableHeader>
                        <TableRow>
                          <TableHead className="h-8 text-xs">{t('tools.skillCol')}</TableHead>
                          <TableHead className="h-8 w-16 text-xs text-right">{t('tools.actionCol')}</TableHead>
                        </TableRow>
                      </TableHeader>
                      <TableBody>
                        {linkedSkills.map((skillName) => (
                          <TableRow key={skillName}>
                            <TableCell className="py-1.5 text-sm">{skillName}</TableCell>
                            <TableCell className="py-1.5 text-right">
                              <Button
                                variant="ghost"
                                size="icon"
                                className="h-7 w-7"
                                disabled={operating === `${tool.id}-unlink-${skillName}`}
                                onClick={() => handleUnlink(skillName, tool.id)}
                                title={`Unlink ${skillName}`}
                              >
                                <Unlink2 className="h-3.5 w-3.5 text-destructive" />
                              </Button>
                            </TableCell>
                          </TableRow>
                        ))}
                      </TableBody>
                    </Table>
                  )}

                  <div className="flex gap-2">
                    <input
                      type="text"
                      placeholder={t('tools.linkPlaceholder')}
                      value={inputValue}
                      onChange={(e) =>
                        setLinkInput((prev) => ({ ...prev, [tool.id]: e.target.value }))
                      }
                      className="h-8 flex-1 rounded-md border border-input bg-background px-3 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                      onKeyDown={(e) => {
                        if (e.key === 'Enter' && inputValue.trim()) {
                          handleLink(inputValue.trim(), tool.id)
                        }
                      }}
                    />
                    <Button
                      size="sm"
                      className="h-8"
                      disabled={!inputValue.trim() || operating === `${tool.id}-link`}
                      onClick={() => {
                        if (inputValue.trim()) {
                          handleLink(inputValue.trim(), tool.id)
                        }
                      }}
                    >
                      <Link2 className="mr-1 h-3.5 w-3.5" />
                      {t('tools.link')}
                    </Button>
                  </div>
                </CardContent>

                <CardFooter className="border-t pt-3 text-xs text-muted-foreground">
                  {tool.is_custom ? t('tools.customTool') : t('tools.systemTool')}
                </CardFooter>
              </Card>
            )
          })}
        </div>
      )}
    </div>
  )
}