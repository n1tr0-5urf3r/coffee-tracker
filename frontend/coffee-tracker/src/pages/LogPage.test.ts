import { describe, expect, it } from 'vitest'
import type { Beverage } from '../api/client'
import { primaryBeveragesFor, secondaryBeveragesFor } from '../lib/beverages'
import { stampCardProgress } from '../lib/stampCard'

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

describe('stampCardProgress', () => {
  const card = (paid: number) => ({ paid_drink_count: paid, stamps_per_card: 12 })

  it('counts stamps on the current card', () => {
    expect(stampCardProgress(card(0))).toMatchObject({ filled: 0, remaining: 12, nextIsFree: false })
    expect(stampCardProgress(card(5))).toMatchObject({ filled: 5, remaining: 7, nextIsFree: false })
  })

  it('announces the free drink once the card is full', () => {
    expect(stampCardProgress(card(12))).toMatchObject({ filled: 12, remaining: 0, nextIsFree: true })
  })

  it('starts a fresh card after the free drink', () => {
    expect(stampCardProgress(card(13))).toMatchObject({ filled: 0, nextIsFree: false })
    expect(stampCardProgress(card(25))).toMatchObject({ filled: 12, nextIsFree: true })
  })
})
