import { useQuery } from '@tanstack/react-query'
import { api } from '../api/client'
import { queryKeys } from '../api/queries'
import { Card, Skeleton } from '../components/Card'
import { countSince, countToday } from '../lib/stats'

export function SimpleStatsPage() {
  const summary = useQuery({ queryKey: queryKeys.summary, queryFn: api.summary })
  const buckets = useQuery({ queryKey: queryKeys.buckets, queryFn: api.buckets })

  return (
    <div>
      <h1 className="mb-6 font-display text-2xl font-black tracking-tight sm:text-3xl">Your stats</h1>
      <div className="grid gap-3 sm:grid-cols-3">
        {buckets.isLoading ? (
          <><Skeleton className="h-28" /><Skeleton className="h-28" /></>
        ) : buckets.isError || !buckets.data ? (
          <Card className="sm:col-span-2" role="alert">
            <p>Recent drink totals could not be loaded.</p>
            <button type="button" className="mt-2 font-bold text-primary underline dark:text-blush" onClick={() => void buckets.refetch()}>Try again</button>
          </Card>
        ) : (
          <>
            <StatCard label="Today" value={countToday(buckets.data)} />
            <StatCard label="Last 7 days" value={countSince(7, buckets.data)} />
          </>
        )}
        {summary.isLoading ? (
          <Skeleton className="h-28" />
        ) : summary.isError || !summary.data ? (
          <Card role="alert">
            <p>All-time total could not be loaded.</p>
            <button type="button" className="mt-2 font-bold text-primary underline dark:text-blush" onClick={() => void summary.refetch()}>Try again</button>
          </Card>
        ) : (
          <StatCard label="All time" value={summary.data.total_drinks} />
        )}
      </div>
    </div>
  )
}

function StatCard({ label, value }: { label: string; value: number }) {
  return (
    <Card className="p-5">
      <p className="text-sm font-bold text-berry-700 dark:text-rose-100">{label}</p>
      <p className="mt-2 font-display text-3xl font-black tabular-nums text-primary dark:text-blush">{value}</p>
    </Card>
  )
}
