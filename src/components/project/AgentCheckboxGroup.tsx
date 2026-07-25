import { useEffect, useState } from 'react'
import { Checkbox } from '@/components/ui/checkbox'
import { getTools } from '@/api/tools'
import type { Tool } from '@/types'

interface AgentCheckboxGroupProps {
  value: string[]
  onChange: (agents: string[]) => void
  disabled?: boolean
}

export function AgentCheckboxGroup({ value, onChange, disabled }: AgentCheckboxGroupProps) {
  const [tools, setTools] = useState<Tool[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    let cancelled = false
    ;(async () => {
      try {
        const data = await getTools()
        if (!cancelled) {
          // 只显示启用的工具
          setTools(data.filter((t) => t.enabled))
        }
      } catch (err) {
        console.error('Failed to load tools:', err)
      } finally {
        if (!cancelled) setLoading(false)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [])

  const handleToggle = (agentValue: string, checked: boolean) => {
    if (checked) {
      onChange([...value, agentValue])
    } else {
      onChange(value.filter((v) => v !== agentValue))
    }
  }

  if (loading) {
    return (
      <div className="space-y-3">
        {[1, 2, 3].map((i) => (
          <div key={i} className="flex items-start space-x-3">
            <div className="h-4 w-4 animate-pulse rounded bg-gray-200 dark:bg-gray-700" />
            <div className="flex-1 space-y-1">
              <div className="h-4 w-24 animate-pulse rounded bg-gray-200 dark:bg-gray-700" />
              <div className="h-3 w-32 animate-pulse rounded bg-gray-200 dark:bg-gray-700" />
            </div>
          </div>
        ))}
      </div>
    )
  }

  if (tools.length === 0) {
    return (
      <div className="text-sm text-muted-foreground">
        暂无可用工具，请在设置中配置。
      </div>
    )
  }

  return (
    <div className="space-y-3">
      {tools.map((tool) => {
        const isChecked = value.includes(tool.id)
        return (
          <div key={tool.id} className="flex items-start space-x-3">
            <Checkbox
              id={`agent-${tool.id}`}
              checked={isChecked}
              onCheckedChange={(checked) => handleToggle(tool.id, checked === true)}
              disabled={disabled}
              className="mt-0.5"
            />
            <label
              htmlFor={`agent-${tool.id}`}
              className={`flex-1 cursor-pointer ${disabled ? 'opacity-50' : ''}`}
            >
              <div className="font-medium text-sm text-foreground">{tool.name}</div>
              <div className="text-xs text-muted-foreground">{tool.path}</div>
            </label>
          </div>
        )
      })}
    </div>
  )
}
