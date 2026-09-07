import { describe, expect, it } from 'vitest'
import type { Beverage } from '../api/client'
import { primaryBeveragesFor, secondaryBeveragesFor } from '../lib/beverages'

const beverages: Beverage[] = [
  { id: 'coffee', name: 'Coffee (Vollautomat)', price_cents: 50, caffeine_mg: 90 },
  { id: 'espresso_pads', name: 'Espresso (pads)', price_cents: 50, caffeine_mg: 90 },
  { id: 'eis', name: 'Eis', price_cents: 50, caffeine_mg: 0 },
  { id: 'tea', name: 'Tea', price_cents: 25, caffeine_mg: 0 },
  { id: 'other', name: 'Other', price_cents: 100, caffeine_mg: 0 },
]

describe('beverage grouping', () => {
  it('keeps coffee and espresso as primary actions before secondary options', () => {
    expect(primaryBeveragesFor(beverages).map((beverage) => beverage.id)).toEqual([
      'coffee',
      'espresso_pads',
    ])
    expect(secondaryBeveragesFor(beverages).map((beverage) => beverage.id)).toEqual([
      'eis',
      'tea',
      'other',
    ])
  })
})
