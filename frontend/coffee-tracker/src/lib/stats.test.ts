import { describe, expect, it } from 'vitest'
import {
  beverageBreakdown,
  caffeineCurve,
  countSince,
  countToday,
  rangeBuckets,
  todayCountsByBeverage,
} from './stats'

describe('dashboard derivations', () => {
  it('counts today from buckets', () => {
    const date = new Date().toISOString().slice(0, 10)
    expect(
      countToday({ buckets: [{ date, count: 2, price_cents: 100 }] }),
    ).toBe(2)
  })

  it('trims daily buckets to the selected range', () => {
    const buckets = Array.from({ length: 45 }, (_, index) => index)
    expect(rangeBuckets(buckets, 'all')).toHaveLength(45)
    expect(rangeBuckets(buckets, '30')).toHaveLength(30)
    expect(rangeBuckets(buckets, '30')[29]).toBe(44)
    expect(rangeBuckets([1, 2], '30')).toEqual([1, 2])
  })

  it('counts rolling bucket windows', () => {
    const today = new Date()
    const yesterday = new Date()
    yesterday.setDate(today.getDate() - 1)
    expect(
      countSince(7, {
        buckets: [
          {
            date: today.toISOString().slice(0, 10),
            count: 1,
            price_cents: 50,
          },
          {
            date: yesterday.toISOString().slice(0, 10),
            count: 3,
            price_cents: 150,
          },
        ],
      }),
    ).toBe(4)
  })

  it('creates a 24 hour caffeine curve plus current point', () => {
    expect(
      caffeineCurve({
        caffeine_mg: 100,
        half_life_hours: 5,
        calculated_at: 0,
      }),
    ).toHaveLength(25)
  })

  it('counts today per beverage including optimistic taps', () => {
    const now = new Date('2026-04-29T12:00:00.000Z')
    const counts = todayCountsByBeverage(
      [
        {
          beverage_id: 'coffee',
          consumed_at: Date.parse('2026-04-29T08:00:00.000Z') / 1000,
        },
        {
          beverage_id: 'espresso_pads',
          consumed_at: Date.parse('2026-04-29T09:00:00.000Z') / 1000,
        },
      ],
      'espresso_pads',
      now,
    )

    expect(counts.get('coffee')).toBe(1)
    expect(counts.get('espresso_pads')).toBe(2)
  })

  it('builds beverage split entries from all summary beverages', () => {
    expect(
      beverageBreakdown({
        total_drinks: 3,
        coffee_count: 1,
        other_count: 0,
        beverages: [
          { beverage_id: 'coffee', name: 'Coffee', count: 1, price_cents: 50 },
          { beverage_id: 'espresso_pads', name: 'Espresso', count: 1, price_cents: 50 },
          { beverage_id: 'eis', name: 'Eis', count: 1, price_cents: 50 },
        ],
        recent_consumptions: [],
        total_price_cents: 150,
        current_streak_days: 1,
        longest_streak_days: 1,
        caffeine_mg: 180,
      }).map((entry) => entry.name),
    ).toEqual(['Coffee', 'Espresso', 'Eis'])
  })
})
