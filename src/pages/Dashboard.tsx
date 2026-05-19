import { useNavigate } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { Package, Wrench } from 'lucide-react'
import { useLibrary } from '@/hooks/use-library'
import { useTools } from '@/hooks/use-tools'
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'

export default function Dashboard() {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const { skillNames, loading: libraryLoading, error: libraryError, refresh: refreshLibrary, scan } = useLibrary()
  const { tools, loading: toolsLoading, error: toolsError, refresh: refreshTools } = useTools()

  const isLoading = libraryLoading || toolsLoading
  const hasError = libraryError || toolsError

  const handleRetry = () => {
    refreshLibrary()
    refreshTools()
  }

  const handleScan = async () => {
    await scan()
    navigate('/scanner?tab=scan')
  }

  if (hasError && !isLoading) {
    return (
      <div className="flex flex-col items-center justify-center gap-4 py-16">
        <p className="text-sm text-destructive">
          {libraryError?.message || toolsError?.message || t('dashboard.loadFailed')}
        </p>
        <Button variant="outline" onClick={handleRetry}>
          {t('error.retry')}
        </Button>
      </div>
    )
  }

  const stats = [
    {
      title: t('dashboard.skills'),
      value: skillNames.length,
      icon: Package,
      description: t('dashboard.skillsDesc'),
    },
    {
      title: t('dashboard.agents'),
      value: tools.length,
      icon: Wrench,
      description: t('dashboard.agentsDesc'),
    },
  ]

  return (
    <div className="space-y-8">
      <div>
        <h2 className="text-2xl font-semibold text-gray-900 dark:text-gray-100">
          {t('dashboard.title')}
        </h2>
        <p className="mt-1 text-sm text-muted-foreground">
          {t('dashboard.subtitle')}
        </p>
      </div>

      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {stats.map((stat) => {
          const Icon = stat.icon
          return (
            <Card key={stat.title}>
              <CardHeader className="flex flex-row items-center justify-between pb-2">
                <CardTitle className="text-sm font-medium text-muted-foreground">
                  {stat.title}
                </CardTitle>
                <Icon className="h-4 w-4 text-muted-foreground" />
              </CardHeader>
              <CardContent>
                {isLoading ? (
                  <Skeleton className="h-8 w-16" />
                ) : (
                  <>
                    <div className="text-2xl font-bold">{stat.value}</div>
                    <p className="text-xs text-muted-foreground">{stat.description}</p>
                  </>
                )}
              </CardContent>
            </Card>
          )
        })}
      </div>

      <div>
        <h3 className="mb-3 text-lg font-medium text-gray-900 dark:text-gray-100">
          {t('dashboard.quickActions')}
        </h3>
        <div className="flex flex-wrap gap-3">
          <Button onClick={handleScan}>
            <Package className="mr-2 h-4 w-4" />
            {t('dashboard.scanForSkills')}
          </Button>
          <Button variant="outline" onClick={() => navigate('/settings')}>
            <Wrench className="mr-2 h-4 w-4" />
            {t('dashboard.manageAgents')}
          </Button>
          <Button variant="outline" onClick={() => navigate('/library')}>
            <Package className="mr-2 h-4 w-4" />
            {t('dashboard.browseLibrary')}
          </Button>
        </div>
      </div>

    </div>
  )
}