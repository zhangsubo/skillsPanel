import { MoreVertical, Edit, Trash2, Upload } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Button } from '@/components/ui/button'

interface SkillActionMenuProps {
  skillName: string
  agents: string[]
  onEditAgents: () => void
  onDelete: () => void
  onImportToCenter: () => void
}

export function SkillActionMenu({
  onEditAgents,
  onDelete,
  onImportToCenter,
}: SkillActionMenuProps) {
  const { t } = useTranslation()

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="ghost"
          size="icon"
          className="h-8 w-8"
          onClick={(e) => e.stopPropagation()}
        >
          <MoreVertical className="h-4 w-4" />
          <span className="sr-only">{t('project.skillActions')}</span>
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem onClick={onEditAgents}>
          <Edit className="mr-2 h-4 w-4" />
          {t('project.editAgents')}
        </DropdownMenuItem>
        <DropdownMenuItem onClick={onImportToCenter}>
          <Upload className="mr-2 h-4 w-4" />
          {t('project.importToCenter')}
        </DropdownMenuItem>
        <DropdownMenuItem onClick={onDelete} className="text-destructive focus:text-destructive">
          <Trash2 className="mr-2 h-4 w-4" />
          {t('project.deleteSkill')}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
