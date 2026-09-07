import { describe, expect, it } from 'vitest'
import { formatDate, formatUnixDate } from './date'

describe('date helpers', () => {
  it('formats dates with German day-month-year order', () => {
    expect(formatDate(Date.UTC(2026, 4, 7, 12))).toBe('07.05.2026')
  })

  it('formats unix timestamps and keeps fallback handling', () => {
    expect(formatUnixDate(Date.UTC(2026, 4, 7, 12) / 1000)).toBe('07.05.2026')
    expect(formatUnixDate(null, 'Never')).toBe('Never')
  })
})
