import { describe, test, expect } from 'vitest'
import { isNewerVersion } from './version'

describe('isNewerVersion', () => {
  test('returns true when latest is greater', () => {
    expect(isNewerVersion('0.2.6', '0.2.7')).toBe(true)
    expect(isNewerVersion('0.2.6', '0.3.0')).toBe(true)
    expect(isNewerVersion('0.2.6', '1.0.0')).toBe(true)
  })

  test('returns false when latest is equal', () => {
    expect(isNewerVersion('0.2.6', '0.2.6')).toBe(false)
    expect(isNewerVersion('0.2.6', 'v0.2.6')).toBe(false)
  })

  test('returns false when latest is smaller', () => {
    expect(isNewerVersion('0.2.7', '0.2.6')).toBe(false)
    expect(isNewerVersion('1.0.0', '0.9.9')).toBe(false)
  })

  test('handles v prefix', () => {
    expect(isNewerVersion('0.2.6', 'v0.2.7')).toBe(true)
    expect(isNewerVersion('v0.2.6', '0.2.7')).toBe(true)
    expect(isNewerVersion('v0.2.6', 'v0.2.6')).toBe(false)
  })

  test('handles different segment lengths', () => {
    expect(isNewerVersion('0.2', '0.2.1')).toBe(true)
    expect(isNewerVersion('0.2.6', '0.2')).toBe(false)
  })
})
