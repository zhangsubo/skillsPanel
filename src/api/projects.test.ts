import { describe, test, expect, vi, beforeEach } from 'vitest'

// Mock invokeCommand before importing the module under test
vi.mock('./index', () => ({
  invokeCommand: vi.fn(),
}))

import { invokeCommand } from './index'
import { createProject } from './projects'

describe('createProject', () => {
  beforeEach(() => {
    vi.mocked(invokeCommand).mockReset()
  })

  test('returns the id string from backend (根因 3 前端契约)', async () => {
    // 后端 create_project 改 Result<String, AppError> 返回新建项目的 uuid。
    // 前端 createProject 必须透传这个 string，让调用方拿到 id 后 navigate。
    // 当前 createProject 返回 Promise<void>，必红。
    vi.mocked(invokeCommand).mockResolvedValue('uuid-1234-abc')

    const id = await createProject('my-project', '/path/to/project')

    expect(id).toBe('uuid-1234-abc')
    expect(invokeCommand).toHaveBeenCalledWith('create_project', {
      name: 'my-project',
      rootPath: '/path/to/project',
    })
  })

  test('trims/preserves the same args the user typed', async () => {
    vi.mocked(invokeCommand).mockResolvedValue('uuid-2')

    await createProject('spaced name', '/with space/')

    expect(invokeCommand).toHaveBeenCalledWith('create_project', {
      name: 'spaced name',
      rootPath: '/with space/',
    })
  })
})
