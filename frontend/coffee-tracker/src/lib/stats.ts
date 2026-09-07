import type {
  BucketsResponse,
  CaffeineResponse,
  BeverageSummary,
  HourlyResponse,
  SummaryResponse,
} from '../api/client'

export function todayKey(date = new Date()) {
  return date.toISOString().slice(0, 10)
}

export function countSince(days: number, buckets?: BucketsResponse) {
  if (!buckets) return 0
  const cutoff = new Date()
  cutoff.setDate(cutoff.getDate() - days + 1)
  cutoff.setHours(0, 0, 0, 0)
  return buckets.buckets
    .filter((bucket) => new Date(`${bucket.date}T00:00:00`) >= cutoff)
    .reduce((sum, bucket) => sum + bucket.count, 0)
}

/**
 * Personal daily buckets arrive as the full history; the dashboard toggle trims
 * them client-side because the endpoint has no range parameter.
 */
export function rangeBuckets<T>(buckets: T[], range: '30' | 'all') {
  return range === 'all' ? buckets : buckets.slice(-30)
}

export function countToday(buckets?: BucketsResponse) {
  return buckets?.buckets.find((bucket) => bucket.date === todayKey())?.count ?? 0
}

export function todayCountsByBeverage(
  consumptions: { beverage_id: string; consumed_at: number }[],
  optimisticBeverageId?: string | null,
  now = new Date(),
) {
  const today = todayKey(now)
  const counts = new Map<string, number>()
  for (const consumption of consumptions) {
    const date = new Date(consumption.consumed_at * 1000).toISOString().slice(0, 10)
    if (date !== today) continue
    counts.set(consumption.beverage_id, (counts.get(consumption.beverage_id) ?? 0) + 1)
  }
  if (optimisticBeverageId) {
    counts.set(optimisticBeverageId, (counts.get(optimisticBeverageId) ?? 0) + 1)
  }
  return counts
}

export function averageFirstCoffeeLabel(hourly?: HourlyResponse) {
  const firstBusyHour = hourly?.buckets.find((bucket) => bucket.count > 0)
  if (!firstBusyHour) return 'Not brewed yet'
  return `${String(firstBusyHour.hour).padStart(2, '0')}:00-ish`
}

export function caffeineCurve(caffeine?: CaffeineResponse) {
  const current = caffeine?.caffeine_mg ?? 0
  return Array.from({ length: 25 }, (_, index) => {
    const hoursAgo = 24 - index
    return {
      label: `${hoursAgo}h`,
      mg: Math.round(current * Math.pow(2, -hoursAgo / 5)),
    }
  })
}

export function beverageBreakdown(summary?: SummaryResponse) {
  const beverages = summary?.beverages ?? []
  if (beverages.length === 0) {
    return [
      { name: 'Coffee', value: summary?.coffee_count ?? 0, fill: '#991238' },
      { name: 'Other', value: summary?.other_count ?? 0, fill: '#44c7a4' },
    ]
  }
  return beverageBreakdownFromEntries(beverages)
}

export function beverageBreakdownFromEntries(beverages: BeverageSummary[]) {
  return beverages.map((beverage, index) => ({
    name: beverage.name,
    value: beverage.count,
    fill: beverageColors[index % beverageColors.length],
  }))
}

const beverageColors = ['#991238', '#44c7a4', '#ffd166', '#65c7f7', '#ec9aac', '#2c303a']
