import { describe, expect, it } from 'vitest'
import { formatStockValue, stockLabelForBeverage } from './stock'

describe('stock helpers', () => {
  it('formats kg and count stock values', () => {
    expect(formatStockValue(0.5, 'kg')).toBe('0,5 kg')
    expect(formatStockValue(10, 'count')).toBe('10')
  })

  it('describes stock usage for primary beverages', () => {
    expect(stockLabelForBeverage('coffee')).toBe('Uses beans')
    expect(stockLabelForBeverage('espresso_pads')).toBe('Uses 1 pad')
    expect(stockLabelForBeverage('tea')).toBe('Uses loose tea')
  })
})
