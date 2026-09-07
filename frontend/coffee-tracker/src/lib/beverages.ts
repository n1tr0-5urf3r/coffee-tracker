import type { Beverage } from '../api/client'

const primaryBeverageIds = ['coffee', 'espresso_pads']

export function primaryBeveragesFor(beverages: Beverage[]) {
  return beverages.filter((beverage) => primaryBeverageIds.includes(beverage.id))
}

export function secondaryBeveragesFor(beverages: Beverage[]) {
  return beverages.filter((beverage) => !primaryBeverageIds.includes(beverage.id))
}

export function compactBeverageName(beverage: Beverage) {
  if (beverage.id === 'coffee') return 'Coffee (beans)'
  if (beverage.id === 'espresso_pads') return 'Espresso (pads)'
  return beverage.name
}
