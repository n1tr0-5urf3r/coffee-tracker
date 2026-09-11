import { describe, expect, it } from 'vitest'
import { formatMonthLabel, monthKey, shiftMonth } from './month'

describe('month helpers', () => {
  it('pads single-digit months', () => {
    expect(monthKey(new Date(2026, 0, 15))).toBe('2026-01')
    expect(monthKey(new Date(2026, 11, 1))).toBe('2026-12')
  })

  it('rolls over year boundaries in both directions', () => {
    expect(shiftMonth('2026-12', 1)).toBe('2027-01')
    expect(shiftMonth('2026-01', -1)).toBe('2025-12')
    expect(shiftMonth('2026-09', -3)).toBe('2026-06')
  })

  it('formats a readable month label', () => {
    expect(formatMonthLabel('2026-09')).toBe('September 2026')
  })
})
