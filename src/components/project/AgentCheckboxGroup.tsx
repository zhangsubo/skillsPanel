import { Checkbox } from '@/components/ui/checkbox'

interface AgentCheckboxGroupProps {
  value: string[]
  onChange: (agents: string[]) => void
  disabled?: boolean
}

const AGENTS = [
  { value: 'claude-code', label: 'Claude Code', path: '.claude/skills' },
  { value: 'cursor', label: 'Cursor', path: '.cursor/skills' },
  { value: 'opencode', label: 'OpenCode', path: '.config/opencode/skill' },
  { value: 'codex', label: 'Codex', path: '.codex/skills' },
  { value: 'agents', label: 'Agents', path: '.agents/skills' },
]

export function AgentCheckboxGroup({ value, onChange, disabled }: AgentCheckboxGroupProps) {
  const handleToggle = (agentValue: string, checked: boolean) => {
    if (checked) {
      onChange([...value, agentValue])
    } else {
      onChange(value.filter((v) => v !== agentValue))
    }
  }

  return (
    <div className="space-y-3">
      {AGENTS.map((agent) => {
        const isChecked = value.includes(agent.value)
        return (
          <div key={agent.value} className="flex items-start space-x-3">
            <Checkbox
              id={`agent-${agent.value}`}
              checked={isChecked}
              onCheckedChange={(checked) => handleToggle(agent.value, checked === true)}
              disabled={disabled}
              className="mt-0.5"
            />
            <label
              htmlFor={`agent-${agent.value}`}
              className={`flex-1 cursor-pointer ${disabled ? 'opacity-50' : ''}`}
            >
              <div className="font-medium text-sm text-foreground">{agent.label}</div>
              <div className="text-xs text-muted-foreground">{agent.path}</div>
            </label>
          </div>
        )
      })}
    </div>
  )
}
