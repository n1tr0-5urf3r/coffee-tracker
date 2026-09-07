import type { StockItem } from '../api/client'

export function formatStockAmount(item: Pick<StockItem, 'unit' | 'current_amount'>) {
  return formatStockValue(item.current_amount, item.unit)
}

export function formatStockValue(value: number, unit: string) {
  if (unit === 'kg') {
    return `${formatNumber(value)} kg`
  }
  return `${formatNumber(value)}`
}

export function stockLabelForBeverage(beverageId: string) {
  if (beverageId === 'coffee') return 'Uses beans'
  if (beverageId === 'espresso_pads') return 'Uses 1 pad'
  if (beverageId === 'tea') return 'Uses loose tea'
  return 'Shared stock unchanged'
}

function formatNumber(value: number) {
  return new Intl.NumberFormat('de-DE', {
    maximumFractionDigits: 3,
  }).format(value)
}
