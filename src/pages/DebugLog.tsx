import { useState } from 'react'
import { useLogs } from '@/hooks/use-logs'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import { RotateCcw, Send } from 'lucide-react'

const LEVEL_COLORS: Record<string, string> = {
  error: 'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-300',
  warn: 'bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-300',
  info: 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-300',
  debug: 'bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-300',
}

export default function DebugLog() {
  const { logs, loading, refresh, sendLog } = useLogs()
  const [filter, setFilter] = useState('')
  const [manualMessage, setManualMessage] = useState('')

  const filtered = logs.filter((log) => {
    const q = filter.toLowerCase()
    return (
      log.message.toLowerCase().includes(q) ||
      log.level.toLowerCase().includes(q) ||
      log.source.toLowerCase().includes(q)
    )
  })

  return (
    <div className="flex h-full flex-col gap-4">
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-semibold">Debug Logs</h2>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm" onClick={refresh} disabled={loading}>
            <RotateCcw className="mr-1 h-3 w-3" />
            Refresh
          </Button>
        </div>
      </div>

      <div className="flex items-center gap-2">
        <Input
          placeholder="Filter logs..."
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          className="max-w-sm"
        />
        <span className="text-xs text-muted-foreground">
          {filtered.length} / {logs.length} logs
        </span>
      </div>

      <div className="flex items-center gap-2">
        <Input
          placeholder="Send manual log message..."
          value={manualMessage}
          onChange={(e) => setManualMessage(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && manualMessage.trim()) {
              sendLog('info', manualMessage)
              setManualMessage('')
            }
          }}
          className="max-w-md"
        />
        <Button
          size="sm"
          onClick={() => {
            if (manualMessage.trim()) {
              sendLog('info', manualMessage)
              setManualMessage('')
            }
          }}
        >
          <Send className="mr-1 h-3 w-3" />
          Send
        </Button>
      </div>

      <Card className="flex-1 overflow-hidden">
        <CardHeader className="pb-2">
          <CardTitle className="text-sm font-medium">Application Logs</CardTitle>
        </CardHeader>
        <CardContent className="h-full overflow-auto p-0">
          {filtered.length === 0 ? (
            <div className="flex h-32 items-center justify-center text-sm text-muted-foreground">
              {loading ? 'Loading...' : 'No logs found.'}
            </div>
          ) : (
            <table className="w-full text-xs">
              <thead className="sticky top-0 bg-gray-50 dark:bg-gray-900">
                <tr>
                  <th className="px-3 py-2 text-left font-medium text-muted-foreground">Time</th>
                  <th className="px-3 py-2 text-left font-medium text-muted-foreground">Level</th>
                  <th className="px-3 py-2 text-left font-medium text-muted-foreground">Source</th>
                  <th className="px-3 py-2 text-left font-medium text-muted-foreground">Message</th>
                </tr>
              </thead>
              <tbody className="divide-y">
                {[...filtered].reverse().map((log, i) => (
                  <tr key={i} className="hover:bg-muted/50">
                    <td className="whitespace-nowrap px-3 py-1.5 text-muted-foreground">
                      {new Date(log.timestamp).toLocaleTimeString()}
                    </td>
                    <td className="px-3 py-1.5">
                      <Badge
                        variant="secondary"
                        className={LEVEL_COLORS[log.level] || LEVEL_COLORS.debug}
                      >
                        {log.level}
                      </Badge>
                    </td>
                    <td className="whitespace-nowrap px-3 py-1.5 text-muted-foreground">
                      {log.source}
                    </td>
                    <td className="px-3 py-1.5 break-all font-mono">{log.message}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
