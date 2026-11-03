import { describe, expect, it } from 'vitest'
import { easterSunday, parseSeasonOverride, resolveSeason } from './seasons'

const on = (year: number, month: number, day: number) => resolveSeason(new Date(year, month - 1, day, 12))

describe('easterSunday', () => {
  it.each([
    [2024, 3, 31],
    [2025, 4, 20],
    [2026, 4, 5],
    [2027, 3, 28],
    [2038, 4, 25],
  ])('computes Easter %i', (year, month, day) => {
    const easter = easterSunday(year)
    expect([easter.getFullYear(), easter.getMonth() + 1, easter.getDate()]).toEqual([year, month, day])
  })
})

describe('resolveSeason', () => {
  it('handles the year change', () => {
    expect(on(2026, 12, 26)).toBe('christmas')
    expect(on(2026, 12, 27)).toBe('winter')
    expect(on(2026, 12, 30)).toBe('winter')
    expect(on(2026, 12, 31)).toBe('newyear')
    expect(on(2027, 1, 2)).toBe('newyear')
    expect(on(2027, 1, 3)).toBe('winter')
  })

  it('starts Advent on December 1st', () => {
    expect(on(2026, 11, 30)).toBe('autumn')
    expect(on(2026, 12, 1)).toBe('christmas')
  })

  it('switches from winter to spring in March', () => {
    expect(on(2028, 2, 29)).toBe('winter')
    expect(on(2027, 2, 28)).toBe('winter')
    expect(on(2027, 3, 1)).toBe('spring')
  })

  it('lets Easter win over spring, from Palm Sunday to Easter Monday', () => {
    expect(on(2026, 3, 28)).toBe('spring')
    expect(on(2026, 3, 29)).toBe('easter')
    expect(on(2026, 4, 5)).toBe('easter')
    expect(on(2026, 4, 6)).toBe('easter')
    expect(on(2026, 4, 7)).toBe('spring')
  })

  it('covers summer and autumn', () => {
    expect(on(2026, 5, 31)).toBe('spring')
    expect(on(2026, 6, 1)).toBe('summer')
    expect(on(2026, 8, 31)).toBe('summer')
    expect(on(2026, 9, 1)).toBe('autumn')
  })

  it('celebrates Coffee Day and Halloween', () => {
    expect(on(2026, 9, 30)).toBe('autumn')
    expect(on(2026, 10, 1)).toBe('coffeeday')
    expect(on(2026, 10, 2)).toBe('autumn')
    expect(on(2026, 10, 23)).toBe('autumn')
    expect(on(2026, 10, 24)).toBe('halloween')
    expect(on(2026, 10, 28)).toBe('halloween')
    expect(on(2026, 10, 29)).toBe('internetday')
    expect(on(2026, 10, 30)).toBe('halloween')
    expect(on(2026, 11, 1)).toBe('halloween')
    expect(on(2026, 11, 2)).toBe('autumn')
  })
})

describe('parseSeasonOverride', () => {
  it('accepts known seasons only', () => {
    expect(parseSeasonOverride('?season=halloween')).toBe('halloween')
    expect(parseSeasonOverride('?foo=1&season=easter')).toBe('easter')
    expect(parseSeasonOverride('?season=pirates')).toBeNull()
    expect(parseSeasonOverride('')).toBeNull()
  })
})
