import type { ReactNode } from 'react'
import { useQuery } from '@tanstack/react-query'
import { CloudOff, RefreshCw } from 'lucide-react'
import { api, isBackendUnavailableError } from '../api/client'
import { Button } from './Button'
import { LogoMark } from './LogoMark'

const HEALTH_KEY = ['backend-health'] as const
const POLL_WHEN_OFFLINE_MS = 10_000
const POLL_WHEN_ONLINE_MS = 60_000

export type BackendStatus = 'checking' | 'online' | 'offline'

export function useBackendHealth() {
  const query = useQuery({
    queryKey: HEALTH_KEY,
    queryFn: api.version,
    retry: false,
    refetchOnWindowFocus: true,
    refetchInterval: (q) =>
      q.state.error ? POLL_WHEN_OFFLINE_MS : POLL_WHEN_ONLINE_MS,
    refetchIntervalInBackground: false,
  })

  const isOffline =
    query.isError && isBackendUnavailableError(query.error)
  const status: BackendStatus = isOffline
    ? 'offline'
    : query.data
      ? 'online'
      : 'checking'

  return {
    status,
    lastCheckedAt: query.dataUpdatedAt || query.errorUpdatedAt,
    isFetching: query.isFetching,
    refetch: () => query.refetch(),
  }
}

export function BackendHealthBoundary({ children }: { children: ReactNode }) {
  const { status, lastCheckedAt, isFetching, refetch } = useBackendHealth()

  if (status === 'offline') {
    return <BackendUnreachablePage lastCheckedAt={lastCheckedAt} isFetching={isFetching} onRetry={refetch} />
  }

  return <>{children}</>
}

function BackendUnreachablePage({
  lastCheckedAt,
  isFetching,
  onRetry,
}: {
  lastCheckedAt: number
  isFetching: boolean
  onRetry: () => void
}) {
  const lastChecked = lastCheckedAt
    ? new Date(lastCheckedAt).toLocaleTimeString()
    : 'never'

  return (
    <div className="grid min-h-svh place-items-center bg-surface p-6 dark:bg-berry-950">
      <div className="w-full max-w-md rounded-4xl border-2 border-berry-100 bg-white p-8 text-center shadow-soft dark:border-white/10 dark:bg-berry-900">
        <LogoMark className="mx-auto" />
        <div className="mt-6 inline-flex items-center gap-2 rounded-full bg-rose-100 px-4 py-1 text-xs font-black uppercase tracking-[0.16em] text-rose-700 dark:bg-rose-500/15 dark:text-rose-200">
          <CloudOff className="size-4" />
          Backend unreachable
        </div>
        <h1 className="mt-5 font-display text-2xl font-black">
          The coffee server is taking a break.
        </h1>
        <p className="mt-3 text-sm font-bold text-berry-700 dark:text-rose-100">
          We can&apos;t reach the API right now. We&apos;ll keep checking
          automatically — every {POLL_WHEN_OFFLINE_MS / 1000}s.
        </p>
        <p className="mt-2 text-xs font-bold text-berry-500 dark:text-rose-200/70">
          Last attempt: {lastChecked}
        </p>
        <Button
          className="mt-6"
          onClick={onRetry}
          disabled={isFetching}
          aria-label="Retry now"
        >
          <RefreshCw
            className={`size-4 ${isFetching ? 'animate-spin' : ''}`}
            aria-hidden="true"
          />
          {isFetching ? 'Retrying…' : 'Retry now'}
        </Button>
      </div>
    </div>
  )
}
