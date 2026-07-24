import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { X } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { AgentCheckboxGroup } from './AgentCheckboxGroup'
import { updateProjectSkillAgents } from '@/api/projects'

interface EditSkillAgentsDialogProps {
  projectId: string
  skillName: string
  currentAgent: string
  onClose: () => void
  onUpdated: () => void
}

export function EditSkillAgentsDialog({
  projectId,
  skillName,
  currentAgent,
  onClose,
  onUpdated,
}: EditSkillAgentsDialogProps) {
  const { t } = useTranslation()
  const [selectedAgents, setSelectedAgents] = useState<string[]>([currentAgent])
  const [submitting, setSubmitting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const handleSubmit = async () => {
    if (selectedAgents.length === 0) {
      setError('请至少选择一个工具')
      return
    }

    setSubmitting(true)
    setError(null)

    try {
      const result = await updateProjectSkillAgents(
        projectId,
        skillName,
        [currentAgent],
        selectedAgents
      )

      // 显示结果
      const messages = []
      if (result.added.length > 0) {
        messages.push(`新增: ${result.added.join(', ')}`)
      }
      if (result.removed.length > 0) {
        messages.push(`删除: ${result.removed.join(', ')}`)
      }
      if (result.unchanged.length > 0) {
        messages.push(`保持: ${result.unchanged.join(', ')}`)
      }

      alert(t('project.agentsUpdated', { added: result.added.length, removed: result.removed.length }) + '\n\n' + messages.join('\n'))
      onUpdated()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="flex w-[500px] flex-col rounded-lg bg-white p-6 shadow-xl dark:bg-gray-900">
        <div className="flex items-center justify-between">
          <h3 className="text-lg font-semibold text-foreground">
            {t('project.editAgentsTitle', { name: skillName })}
          </h3>
          <button onClick={onClose} className="rounded p-1 text-muted-foreground hover:bg-muted">
            <X className="h-4 w-4" />
          </button>
        </div>

        <p className="mt-2 text-sm text-muted-foreground">
          {t('project.editAgentsDesc')}
        </p>

        <div className="mt-4">
          <AgentCheckboxGroup
            value={selectedAgents}
            onChange={setSelectedAgents}
            disabled={submitting}
          />
        </div>

        <div className="mt-4 text-sm text-muted-foreground">
          {t('project.currentAgents')}: <span className="font-medium">{currentAgent}</span>
        </div>

        {error && (
          <p className="mt-3 text-sm text-red-600 dark:text-red-400">{error}</p>
        )}

        <div className="mt-6 flex items-center justify-end gap-2">
          <Button variant="outline" onClick={onClose} disabled={submitting}>
            {t('library.cancel')}
          </Button>
          <Button onClick={handleSubmit} disabled={submitting || selectedAgents.length === 0}>
            {submitting ? '保存中...' : '保存更改'}
          </Button>
        </div>
      </div>
    </div>
  )
}
