import { describe, expect, it } from 'vitest'
import { Coffee, CupSoda, Leaf, Popsicle } from 'lucide-react'
import { beverageIcon, stampLabel } from './beverageIcons'

describe('beverageIcon', () => {
  it('maps every known beverage to its own icon', () => {
    expect(beverageIcon('coffee')).toBe(Coffee)
    expect(beverageIcon('espresso_pads')).toBe(CupSoda)
    expect(beverageIcon('eis')).toBe(Popsicle)
    expect(beverageIcon('tea')).toBe(Leaf)
  })

  it('falls back to a generic cup for unknown ids', () => {
    expect(beverageIcon('other')).toBe(CupSoda)
    expect(beverageIcon('mate')).toBe(CupSoda)
  })
})

describe('stampLabel', () => {
  it('shortens known beverages and keeps unknown names', () => {
    expect(stampLabel('espresso_pads', 'Espresso (pads)')).toBe('Espresso')
    expect(stampLabel('other', 'Other')).toBe('Other')
  })
})
