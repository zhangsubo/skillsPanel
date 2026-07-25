import { describe, it, expect } from 'vitest'
import type { ProjectSkillInfo } from '@/types'

// 聚合逻辑测试（从 ProjectWorkspace 提取）
interface AggregatedProjectSkill {
  name: string
  description: string
  agents: string[]
  enabled: boolean
  allEnabled: boolean
  sync_status: ProjectSkillInfo['sync_status']
  in_center: boolean
  center_skill_id: string | null
}

function aggregateSkills(skills: ProjectSkillInfo[]): AggregatedProjectSkill[] {
  const skillMap = new Map<string, AggregatedProjectSkill>()

  for (const skill of skills) {
    const existing = skillMap.get(skill.name)
    if (existing) {
      existing.agents.push(skill.agent)
      existing.enabled = existing.enabled || skill.enabled
      existing.allEnabled = existing.allEnabled && skill.enabled
    } else {
      skillMap.set(skill.name, {
        name: skill.name,
        description: skill.description,
        agents: [skill.agent],
        enabled: skill.enabled,
        allEnabled: skill.enabled,
        sync_status: skill.sync_status,
        in_center: skill.in_center,
        center_skill_id: skill.center_skill_id,
      })
    }
  }

  return Array.from(skillMap.values())
}

describe('ProjectWorkspace skill aggregation', () => {
  it('should aggregate same skill with different agents', () => {
    const skills: ProjectSkillInfo[] = [
      {
        name: 'test-skill',
        description: 'Test description',
        relative_path: '.claude/skills/test-skill',
        agent: 'claude-code',
        enabled: true,
        content_hash: 'hash1',
        in_center: true,
        center_skill_id: 'id1',
        sync_status: 'in_sync',
      },
      {
        name: 'test-skill',
        description: 'Test description',
        relative_path: '.cursor/skills/test-skill',
        agent: 'cursor',
        enabled: true,
        content_hash: 'hash1',
        in_center: true,
        center_skill_id: 'id1',
        sync_status: 'in_sync',
      },
    ]

    const result = aggregateSkills(skills)

    expect(result).toHaveLength(1)
    expect(result[0].name).toBe('test-skill')
    expect(result[0].agents).toEqual(['claude-code', 'cursor'])
    expect(result[0].enabled).toBe(true)
    expect(result[0].allEnabled).toBe(true)
  })

  it('should handle partially enabled skills', () => {
    const skills: ProjectSkillInfo[] = [
      {
        name: 'test-skill',
        description: 'Test description',
        relative_path: '.claude/skills/test-skill',
        agent: 'claude-code',
        enabled: true,
        content_hash: 'hash1',
        in_center: true,
        center_skill_id: 'id1',
        sync_status: 'in_sync',
      },
      {
        name: 'test-skill',
        description: 'Test description',
        relative_path: '.cursor/skills-disabled/test-skill',
        agent: 'cursor',
        enabled: false,
        content_hash: 'hash1',
        in_center: true,
        center_skill_id: 'id1',
        sync_status: 'in_sync',
      },
    ]

    const result = aggregateSkills(skills)

    expect(result).toHaveLength(1)
    expect(result[0].enabled).toBe(true) // 至少一个启用
    expect(result[0].allEnabled).toBe(false) // 不是全部启用
  })

  it('should keep different skills separate', () => {
    const skills: ProjectSkillInfo[] = [
      {
        name: 'skill-a',
        description: 'Skill A',
        relative_path: '.claude/skills/skill-a',
        agent: 'claude-code',
        enabled: true,
        content_hash: 'hash1',
        in_center: true,
        center_skill_id: 'id1',
        sync_status: 'in_sync',
      },
      {
        name: 'skill-b',
        description: 'Skill B',
        relative_path: '.claude/skills/skill-b',
        agent: 'claude-code',
        enabled: true,
        content_hash: 'hash2',
        in_center: true,
        center_skill_id: 'id2',
        sync_status: 'in_sync',
      },
    ]

    const result = aggregateSkills(skills)

    expect(result).toHaveLength(2)
    expect(result.find((s) => s.name === 'skill-a')).toBeDefined()
    expect(result.find((s) => s.name === 'skill-b')).toBeDefined()
  })
})
