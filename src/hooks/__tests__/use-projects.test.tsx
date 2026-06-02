import { describe, test, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor, act } from '@testing-library/react'
import { useEffect } from 'react'

// Mock @/api/projects before importing modules that use it
vi.mock('@/api/projects', () => ({
  listProjects: vi.fn(),
  scanProject: vi.fn(),
  createProject: vi.fn(),
  deleteProject: vi.fn(),
}))

import * as projectsApi from '@/api/projects'
import { ProjectsProvider } from '@/hooks/projects-context'
import { useProjects } from '@/hooks/use-projects'
import type { Project, ProjectDto } from '@/types'

// ── Test fixtures ────────────────────────────────────────────────

function makeProject(id: string, name: string): Project {
  return {
    id,
    name,
    root_path: `/tmp/${name}`,
    created_at: '2024-01-01T00:00:00Z',
    updated_at: '2024-01-01T00:00:00Z',
  }
}

function makeProjectDto(project: Project): ProjectDto {
  return {
    project,
    skills: [],
    sync_health: {
      in_sync: 0,
      center_newer: 0,
      project_newer: 0,
      diverged: 0,
      project_only: 0,
      center_only: 0,
    },
  }
}

// ── Probe components ─────────────────────────────────────────────

/** ConsumerA: shows projects length, exposes a button to add a project. */
function ConsumerA({ onReady }: { onReady?: (api: ReturnType<typeof useProjects>) => void }) {
  const api = useProjects()
  useEffect(() => {
    onReady?.(api)
  }, [api])
  return (
    <div>
      <span data-testid="A-count">{api.projects.length}</span>
      <button data-testid="A-add" onClick={() => api.addProject('p1', '/tmp/p1')}>
        add
      </button>
      <span data-testid="A-id">{String(api.addProject.length)}</span>
    </div>
  )
}

/** ConsumerB: only shows projects length, no add. Lives under same provider. */
function ConsumerB() {
  const { projects } = useProjects()
  return <span data-testid="B-count">{projects.length}</span>
}

// ── Tests ────────────────────────────────────────────────────────

describe('useProjects (Context 化)', () => {
  beforeEach(() => {
    vi.mocked(projectsApi.listProjects).mockReset()
    vi.mocked(projectsApi.createProject).mockReset()
    vi.mocked(projectsApi.scanProject).mockReset()
    vi.mocked(projectsApi.deleteProject).mockReset()
  })

  test('根因 2 回归：两个消费组件共享同一份 projects state', async () => {
    // 修前：useProjects 在两个组件里 useState 独立，A addProject 只更新 A 的 state
    //       → B-count 仍是 0
    // 修后：Context 共享 state → A addProject → B-count 同步从 0 变 1
    vi.mocked(projectsApi.listProjects)
      .mockResolvedValueOnce([]) // 首次 refresh：空
      .mockResolvedValue([makeProject('uuid-new-1', 'p1')]) // 后续 refresh：包含新加
    vi.mocked(projectsApi.createProject).mockResolvedValue('uuid-new-1')

    render(
      <ProjectsProvider>
        <ConsumerA />
        <ConsumerB />
      </ProjectsProvider>,
    )

    // 初始：listProjects 返回空数组 → 两边都 0
    await waitFor(() => expect(screen.getByTestId('A-count')).toHaveTextContent('0'))
    expect(screen.getByTestId('B-count')).toHaveTextContent('0')

    // A add
    await act(async () => {
      screen.getByTestId('A-add').click()
    })

    // B 立刻看到 1（不是 0）
    await waitFor(() => expect(screen.getByTestId('B-count')).toHaveTextContent('1'))
    expect(screen.getByTestId('A-count')).toHaveTextContent('1')
  })

  test('根因 3 前端契约：addProject 返回新建项目的 id', async () => {
    vi.mocked(projectsApi.listProjects).mockResolvedValue([])
    vi.mocked(projectsApi.createProject).mockResolvedValue('uuid-xyz')

    let capturedId: unknown = undefined
    function Probe() {
      const { addProject } = useProjects()
      return (
        <button
          data-testid="probe"
          onClick={async () => {
            capturedId = await addProject('n', '/p')
          }}
        >
          go
        </button>
      )
    }

    render(
      <ProjectsProvider>
        <Probe />
      </ProjectsProvider>,
    )

    await act(async () => {
      screen.getByTestId('probe').click()
    })

    await waitFor(() => expect(capturedId).toBe('uuid-xyz'))
  })

  test('addProject 后 selectProject 能选到刚加的项目', async () => {
    vi.mocked(projectsApi.listProjects).mockResolvedValue([])
    vi.mocked(projectsApi.createProject).mockResolvedValue('uuid-new-2')
    const dto = makeProjectDto(makeProject('uuid-new-2', 'p2'))
    vi.mocked(projectsApi.scanProject).mockResolvedValue(dto)

    let apiRef: ReturnType<typeof useProjects> | null = null
    function Probe() {
      const api = useProjects()
      useEffect(() => {
        apiRef = api
      }, [api])
      return <span data-testid="probe">{api.projectDetail?.project.name ?? 'none'}</span>
    }

    render(
      <ProjectsProvider>
        <Probe />
      </ProjectsProvider>,
    )

    await waitFor(() => expect(apiRef).not.toBeNull())

    await act(async () => {
      await apiRef!.addProject('p2', '/tmp/p2')
    })

    await act(async () => {
      await apiRef!.selectProject('uuid-new-2')
    })

    await waitFor(() =>
      expect(screen.getByTestId('probe')).toHaveTextContent('p2'),
    )
  })
})
