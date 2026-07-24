import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { X } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { AgentCheckboxGroup } from './AgentCheckboxGroup'
import { updateProjectSkillAgents } from '@/api/projects'
import type { ProjectSkillInfo } from '@/types'

interface BatchEditAgentsDialogProps {
  projectId: string
  skills: ProjectSkillInfo[]
  onClose: () => void
  onUpdated: () => void
}

export function BatchEditAgentsDialog({
  projectId,
  skills,
  onClose,
  onUpdated,
}: BatchEditAgentsDialogProps) {
  const { t } = useTranslation()
  const [selectedAgents, setSelectedAgents] = useState<string[]>(['claude-code'])
  const [submitting, setSubmitting] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [progress, setProgress] = useState({ current: 0, total: 0 })

  const handleSubmit = async () => {
    if (selectedAgents.length === 0) {
      setError('请至少选择一个工具')
      return
    }

    setSubmitting(true)
    setError(null)
    setProgress({ current: 0, total: skills.length })

    try {
      let totalAdded = 0
      let totalRemoved = 0

      for (let i = 0; i < skills.length; i++) {
        const skill = skills[i]
        setProgress({ current: i + 1, total: skills.length })

        const result = await updateProjectSkillAgents(
          projectId,
          skill.name,
          [skill.agent],
          selectedAgents
        )
        totalAdded += result.added.length
        totalRemoved += result.removed.length
      }

      alert(
        `批量修改完成！\n\n` +
        `处理了 ${skills.length} 个 Skill\n` +
        `新增: ${totalAdded} 个安装\n` +
        `删除: ${totalRemoved} 个安装`
      )
      onUpdated()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setSubmitting(false)
      setProgress({ current: 0, total: 0 })
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="flex w-[500px] flex-col rounded-lg bg-white p-6 shadow-xl dark:bg-gray-900">
        <div className="flex items-center justify-between">
          <h3 className="text-lg font-semibold text-foreground">
            {t('project.batchEditAgentsTitle')}
          </h3>
          <button onClick={onClose} className="rounded p-1 text-muted-foreground hover:bg-muted">
            <X className="h-4 w-4" />
          </button>
        </div>

        <p className="mt-2 text-sm text-muted-foreground">
          {t('project.batchEditAgentsDesc', { count: skills.length })}
        </p>

        <div className="mt-3 max-h-32 overflow-y-auto rounded-md border border-gray-200 bg-gray-50 p-2 dark:border-gray-700 dark:bg-gray-800">
          <div className="text-xs text-muted-foreground">
            {skills.map((skill) => (
              <div key={skill.name} className="flex items-center justify-between py-1">
                <span>{skill.name}</span>
                <span className="text-[10px] opacity-60">{skill.agent}</span>
              </div>
            ))}
          </div>
        </div>

        <div className="mt-4">
          <AgentCheckboxGroup
            value={selectedAgents}
            onChange={setSelectedAgents}
            disabled={submitting}
          />
        </div>

        <div className="mt-4 text-sm text-muted-foreground">
          已选择 {skills.length} 个 Skill
        </div>

        {error && (
          <p className="mt-3 text-sm text-red-600 dark:text-red-400">{error}</p>
        )}

        {submitting && progress.total > 0 && (
          <div className="mt-3">
            <div className="flex items-center justify-between text-xs text-muted-foreground mb-1">
              <span>处理进度</span>
              <span>{progress.current} / {progress.total}</span>
            </div>
            <div className="h-2 w-full bg-gray-200 rounded-full overflow-hidden dark:bg-gray-700">
              <div
                className="h-full bg-primary transition-all duration-300"
                style={{ width: `${(progress.current / progress.total) * 100}%` }}
              />
            </div>
          </div>
        )}

        <div className="mt-6 flex items-center justify-end gap-2">
          <Button variant="outline" onClick={onClose} disabled={submitting}>
            {t('library.cancel')}
          </Button>
          <Button onClick={handleSubmit} disabled={submitting || selectedAgents.length === 0}>
            {submitting ? `处理中... (${progress.current}/${progress.total})` : '应用到全部'}
          </Button>
        </div>
      </div>
    </div>
  )
}
