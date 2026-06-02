import { useState } from 'react'
import { Outlet } from 'react-router-dom'
import Sidebar from './Sidebar'
import UpdateDialog from './UpdateDialog'
import { useUpdateCheck } from '@/hooks/use-update-check'
import { ProjectsProvider } from '@/hooks/projects-context'

export default function Layout() {
  const { updateInfo, shouldShow, dismiss } = useUpdateCheck()
  const [dialogOpen, setDialogOpen] = useState(true)

  const handleClose = () => {
    setDialogOpen(false)
    if (updateInfo?.latestVersion) {
      dismiss(updateInfo.latestVersion)
    }
  }

  return (
    <ProjectsProvider>
      <div className="flex h-screen overflow-hidden bg-gray-50 dark:bg-gray-950">
        <Sidebar />
        <main className="flex-1 overflow-y-auto p-6">
          <Outlet />
        </main>
        {shouldShow && updateInfo?.latestVersion && (
          <UpdateDialog
            open={dialogOpen}
            onClose={handleClose}
            currentVersion={updateInfo.currentVersion}
            latestVersion={updateInfo.latestVersion}
          />
        )}
      </div>
    </ProjectsProvider>
  )
}
