import { useState, useEffect, useRef } from 'react'
import { Check, ChevronDown } from 'lucide-react'
import { getTools } from '@/api/tools'
import type { Tool } from '@/types'

interface AgentMultiSelectProps {
  value: string[]
  onChange: (agents: string[]) => void
  disabled?: boolean
}

export function AgentMultiSelect({ value, onChange, disabled }: AgentMultiSelectProps) {
  const [tools, setTools] = useState<Tool[]>([])
  const [loading, setLoading] = useState(true)
  const [isOpen, setIsOpen] = useState(false)
  const dropdownRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    let cancelled = false
    ;(async () => {
      try {
        const data = await getTools()
        if (!cancelled) {
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

  // 点击外部关闭下拉
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setIsOpen(false)
      }
    }

    if (isOpen) {
      document.addEventListener('mousedown', handleClickOutside)
    }

    return () => {
      document.removeEventListener('mousedown', handleClickOutside)
    }
  }, [isOpen])

  const handleToggle = (toolId: string) => {
    if (value.includes(toolId)) {
      onChange(value.filter((v) => v !== toolId))
    } else {
      onChange([...value, toolId])
    }
  }

  const selectedTools = tools.filter((t) => value.includes(t.id))
  const displayText = selectedTools.length === 0
    ? '选择工具...'
    : selectedTools.map((t) => t.name).join(', ')

  if (loading) {
    return (
      <div className="h-10 w-full animate-pulse rounded-md border border-gray-200 bg-gray-100 dark:border-gray-700 dark:bg-gray-800" />
    )
  }

  if (tools.length === 0) {
    return (
      <div className="rounded-md border border-gray-200 bg-gray-50 px-3 py-2 text-sm text-muted-foreground dark:border-gray-700 dark:bg-gray-800">
        暂无可用工具，请在设置中配置。
      </div>
    )
  }

  return (
    <div className="relative" ref={dropdownRef}>
      <button
        type="button"
        onClick={() => !disabled && setIsOpen(!isOpen)}
        disabled={disabled}
        className={`flex w-full items-center justify-between rounded-md border border-gray-200 bg-white px-3 py-2 text-sm text-foreground transition-colors hover:bg-gray-50 dark:border-gray-700 dark:bg-gray-900 dark:hover:bg-gray-800 ${
          disabled ? 'cursor-not-allowed opacity-50' : 'cursor-pointer'
        }`}
      >
        <span className={`flex-1 truncate text-left ${selectedTools.length === 0 ? 'text-muted-foreground' : ''}`}>
          {displayText}
        </span>
        <ChevronDown className={`ml-2 h-4 w-4 shrink-0 transition-transform ${isOpen ? 'rotate-180' : ''}`} />
      </button>

      {isOpen && (
        <div className="absolute left-0 right-0 top-full z-50 mt-1 max-h-60 overflow-y-auto rounded-md border border-gray-200 bg-white shadow-lg dark:border-gray-700 dark:bg-gray-900">
          {tools.map((tool) => {
            const isSelected = value.includes(tool.id)
            return (
              <button
                key={tool.id}
                type="button"
                onClick={() => handleToggle(tool.id)}
                className="flex w-full items-start gap-3 px-3 py-2.5 text-left text-sm transition-colors hover:bg-gray-50 dark:hover:bg-gray-800"
              >
                <div className={`flex h-5 w-5 shrink-0 items-center justify-center rounded border ${
                  isSelected
                    ? 'border-primary bg-primary'
                    : 'border-gray-300 dark:border-gray-600'
                }`}>
                  {isSelected && <Check className="h-3.5 w-3.5 text-white" />}
                </div>
                <div className="flex-1 min-w-0">
                  <div className="font-medium text-foreground">{tool.name}</div>
                  <div className="text-xs text-muted-foreground">{tool.path}</div>
                </div>
              </button>
            )
          })}
        </div>
      )}
    </div>
  )
}
