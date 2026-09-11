import { useEffect, useMemo, useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { AnimatePresence, motion } from 'framer-motion'
import {
  CirclePlus,
  Gift,
  MousePointerClick,
  Stamp,
  RotateCcw,
  WifiOff,
  X,
} from 'lucide-react'
import { api, ApiError, type Beverage, type ConsumptionResponse } from '../api/client'
import { invalidateCoffeeData, useCatalog, useMe, useStampCard, useStats } from '../api/queries'
import { Card, Skeleton } from '../components/Card'
import { PageHeader } from '../components/PageHeader'
import { todayCountsByBeverage } from '../lib/stats'
import { useToast } from '../lib/toast'
import { queueLog } from '../lib/offlineQueue'
import { useOfflineQueue } from '../hooks/useOfflineQueue'
import { cn } from '../lib/cn'
import { stockLabelForBeverage } from '../lib/stock'
import { currentMonthKey } from '../lib/month'
import { stampCardProgress, type StampProgress } from '../lib/stampCard'
import { BeverageIcon } from '../lib/beverageIcons'
import {
  compactBeverageName,
  primaryBeveragesFor,
  secondaryBeveragesFor,
} from '../lib/beverages'

type LastLog = ConsumptionResponse & { beverageName: string }
const cooldownMs = 5000
const undoWindowMs = 30 * 1000

export function LogPage() {
  const queryClient = useQueryClient()
  const { pushToast } = useToast()
  const { data: me } = useMe()
  const { machines, beverages } = useCatalog()
  const stats = useStats()
  const { queuedLogs, refreshQueue, cancelQueuedLog } = useOfflineQueue()
  const [optimisticBeverageId, setOptimisticBeverageId] = useState<string | null>(null)
  const [lastLog, setLastLog] = useState<LastLog | null>(null)
  const [cooldownUntil, setCooldownUntil] = useState<number | null>(null)
  const [clock, setClock] = useState(() => Date.now())

  const params = new URLSearchParams(window.location.search)
  const machineFromUrl = params.get('machine_id') ?? params.get('machine')
  const machineId = machineFromUrl ?? machines.data?.machines[0]?.id ?? 'main'
  const machineName =
    machines.data?.machines.find((machine) => machine.id === machineId)?.name ??
    machineId
  const todayCounts = todayCountsByBeverage(
    stats.summary.data?.recent_consumptions ?? [],
    optimisticBeverageId,
  )

  const logMutation = useMutation({
    mutationFn: async (beverage: Beverage) => {
      if (!navigator.onLine) {
        const queued = await queueLog({
          machine_id: machineId,
          beverage_id: beverage.id,
        })
        return {
          id: queued.id,
          user_id: me?.user.id ?? 'offline',
          machine_id: machineId,
          beverage_id: beverage.id,
          consumed_at: Math.floor(queued.created_at / 1000),
        } satisfies ConsumptionResponse
      }
      return api.createConsumption({
        machine_id: machineId,
        beverage_id: beverage.id,
      })
    },
    onMutate: (beverage) => {
      setOptimisticBeverageId(beverage.id)
    },
    onSuccess: async (data, beverage) => {
      setLastLog({ ...data, beverageName: beverage.name })
      if (!navigator.onLine) {
        await refreshQueue()
        pushToast({
          tone: 'info',
          title: 'Saved for later',
          description: 'No WiFi? No drama. This tap is queued.',
        })
        return
      }
      pushToast({
        tone: 'success',
        title: `${beverage.name} logged`,
        description: 'Tiny tap. Properly counted.',
      })
      await invalidateCoffeeData(queryClient)
      setOptimisticBeverageId(null)
    },
    onError: (error) => {
      setOptimisticBeverageId(null)
      if (error instanceof ApiError && error.status === 409) {
        setCooldownUntil(Date.now() + cooldownMs)
        return
      }
      pushToast({
        tone: 'error',
        title: 'The counter missed that tap',
        description: 'Try once more when the machine has forgiven the WiFi.',
      })
    },
  })

  const undoMutation = useMutation({
    mutationFn: (id: string) => api.deleteConsumption(id),
    onSuccess: async () => {
      setLastLog(null)
      pushToast({
        tone: 'success',
        title: 'Undo complete',
        description: 'That cup has left the tally sheet.',
      })
      await invalidateCoffeeData(queryClient)
    },
  })

  const cooldownSeconds = useMemo(() => {
    if (!cooldownUntil) return 0
    return Math.max(0, Math.ceil((cooldownUntil - clock) / 1000))
  }, [clock, cooldownUntil])
  const cooldownProgress = useMemo(() => {
    if (!cooldownUntil) return 0
    return Math.max(0, Math.min(1, (cooldownUntil - clock) / cooldownMs))
  }, [clock, cooldownUntil])
  const undoUntil = lastLog?.id.startsWith('offline')
    ? null
    : lastLog
      ? lastLog.consumed_at * 1000 + undoWindowMs
      : null
  const undoSeconds = useMemo(() => {
    if (!undoUntil) return 0
    return Math.max(0, Math.ceil((undoUntil - clock) / 1000))
  }, [clock, undoUntil])
  const undoProgress = useMemo(() => {
    if (!undoUntil) return 0
    return Math.max(0, Math.min(1, (undoUntil - clock) / undoWindowMs))
  }, [clock, undoUntil])

  useEffect(() => {
    if (cooldownSeconds === 0 && undoSeconds === 0) return
    const timer = window.setInterval(() => setClock(Date.now()), 250)
    return () => window.clearInterval(timer)
  }, [cooldownSeconds, undoSeconds])

  const primaryBeverages = primaryBeveragesFor(beverages.data?.beverages ?? [])
  const otherBeverages = secondaryBeveragesFor(beverages.data?.beverages ?? [])
  const stampCard = useStampCard(currentMonthKey()).data
  const stampProgress = stampCard ? stampCardProgress(stampCard) : null
  const nextIsFree = stampProgress?.nextIsFree ?? false

  return (
    <div>
      <PageHeader
        title={`Hi ${me?.user.display_name ?? 'there'}, ready?`}
      >
        <div className="flex flex-wrap items-center gap-2">
          <div className="min-w-0 truncate rounded-full bg-white px-4 py-2 text-sm font-black text-berry-700 shadow-soft dark:bg-berry-900 dark:text-rose-100">
            Machine: {machineName}
          </div>
          {stampProgress ? <StampProgressPill progress={stampProgress} /> : null}
        </div>
      </PageHeader>

      {beverages.isLoading || primaryBeverages.length < 2 ? (
        <div className="grid grid-cols-2 gap-3">
          <Skeleton className="h-44" />
          <Skeleton className="h-44" />
        </div>
      ) : (
        <div className="grid grid-cols-2 gap-3">
          {primaryBeverages.map((beverage) => (
            <PrimaryBeverageButton
              key={beverage.id}
              beverage={beverage}
              disabled={logMutation.isPending || cooldownSeconds > 0}
              onClick={() => logMutation.mutate(beverage)}
              todayCount={todayCounts.get(beverage.id) ?? 0}
              free={nextIsFree && beverage.price_cents > 0}
            />
          ))}
        </div>
      )}

      {otherBeverages.length > 0 ? (
        <div className="mt-4 grid gap-3 sm:grid-cols-2">
          {otherBeverages.map((beverage) => (
            <BeverageButton
              key={beverage.id}
              beverage={beverage}
              disabled={logMutation.isPending || cooldownSeconds > 0}
              onClick={() => logMutation.mutate(beverage)}
              free={nextIsFree && beverage.price_cents > 0}
            />
          ))}
        </div>
      ) : null}

      <AnimatePresence>
        {cooldownSeconds > 0 ? (
          <CooldownOverlay seconds={cooldownSeconds} progress={cooldownProgress} />
        ) : null}
      </AnimatePresence>

      <AnimatePresence>
        {lastLog && !lastLog.id.startsWith('offline') && undoSeconds > 0 ? (
          <UndoOverlay
            beverageName={lastLog.beverageName}
            seconds={undoSeconds}
            progress={undoProgress}
            disabled={undoMutation.isPending}
            onClick={() => undoMutation.mutate(lastLog.id)}
          />
        ) : null}
      </AnimatePresence>

      {queuedLogs.length > 0 ? (
        <Card className="mt-5 border-sky bg-sky/15">
          <div className="mb-3 flex items-center gap-2 font-display font-black">
            <WifiOff className="size-5" />
            Queued taps
          </div>
          <div className="space-y-2">
            {queuedLogs.map((log) => (
              <div
                key={log.id}
                className="flex items-center justify-between rounded-2xl bg-white/70 px-3 py-2 dark:bg-white/10"
              >
                <span className="text-sm font-bold">
                  {log.beverage_id} at {new Date(log.created_at).toLocaleTimeString()}
                </span>
                <button
                  type="button"
                  aria-label="Cancel queued log"
                  onClick={() => cancelQueuedLog(log.id)}
                  className="rounded-full p-2 text-primary hover:bg-primary/10"
                >
                  <X className="size-4" />
                </button>
              </div>
            ))}
          </div>
        </Card>
      ) : null}
    </div>
  )
}

function StampProgressPill({ progress }: { progress: StampProgress }) {
  if (progress.nextIsFree) {
    return (
      <div className="flex items-center gap-2 rounded-full bg-mint px-4 py-2 text-sm font-black text-berry-950 shadow-soft">
        <Gift className="size-4 shrink-0" aria-hidden="true" />
        Next drink is free!
      </div>
    )
  }

  return (
    <div className="flex items-center gap-2 rounded-full bg-white px-4 py-2 text-sm font-black text-berry-700 shadow-soft dark:bg-berry-900 dark:text-rose-100">
      <Stamp className="size-4 shrink-0 text-primary dark:text-blush" aria-hidden="true" />
      {progress.filled}/{progress.stampsPerCard} · {progress.remaining} to free
    </div>
  )
}

function PricePill({
  priceCents,
  free,
  tone,
}: {
  priceCents: number
  free: boolean
  tone: 'solid' | 'soft'
}) {
  if (free) {
    return (
      <span className="flex items-center gap-1 rounded-full bg-mint px-3 py-1 text-xs font-black uppercase tracking-[0.08em] text-berry-950 sm:text-sm">
        <Gift className="size-4 shrink-0" aria-hidden="true" />
        Free
      </span>
    )
  }

  return (
    <span
      className={cn(
        'rounded-full px-3 py-1 font-black',
        tone === 'solid'
          ? 'bg-primary text-sm text-white'
          : 'bg-berry-100 text-xs text-berry-700 dark:bg-white/10 dark:text-berry-50',
      )}
    >
      {(priceCents / 100).toFixed(2)} €
    </span>
  )
}

function CooldownOverlay({
  seconds,
  progress,
}: {
  seconds: number
  progress: number
}) {
  return (
    <div className="pointer-events-none fixed inset-x-0 top-20 z-50 flex justify-center px-4 md:top-24">
      <motion.div
        role="status"
        aria-live="polite"
        initial={{ opacity: 0, y: -10, scale: 0.96 }}
        animate={{ opacity: 1, y: 0, scale: 1 }}
        exit={{ opacity: 0, y: -10, scale: 0.96 }}
        className="rounded-full border border-berry-100 bg-[#fffdf8]/95 px-3 py-2 text-berry-950 shadow-soft backdrop-blur dark:border-white/10 dark:bg-berry-900/95 dark:text-berry-50"
      >
        <div className="flex items-center gap-3">
          <CountdownRing seconds={seconds} progress={progress} />
          <div className="pr-2">
            <p className="font-display text-sm font-black">Ready again soon</p>
            <p className="text-xs font-bold text-berry-700 dark:text-rose-100">
              Duplicate taps are blocked for a moment.
            </p>
          </div>
        </div>
      </motion.div>
    </div>
  )
}

function UndoOverlay({
  beverageName,
  seconds,
  progress,
  disabled,
  onClick,
}: {
  beverageName: string
  seconds: number
  progress: number
  disabled: boolean
  onClick: () => void
}) {
  return (
    <div className="pointer-events-none fixed inset-x-0 top-40 z-50 flex justify-center px-4 md:left-[15rem] md:right-4 md:top-44">
      <motion.button
        type="button"
        onClick={onClick}
        disabled={disabled}
        initial={{ opacity: 0, y: -10, scale: 0.96 }}
        animate={{ opacity: 1, y: 0, scale: 1 }}
        exit={{ opacity: 0, y: -10, scale: 0.96 }}
        className="pointer-events-auto flex max-w-[calc(100vw-2rem)] items-center justify-center gap-3 rounded-full border border-mint/35 bg-[#fffdf8]/95 px-3 py-2 text-left text-berry-950 shadow-soft backdrop-blur transition hover:border-mint hover:bg-white disabled:pointer-events-none disabled:opacity-55 dark:border-white/10 dark:bg-berry-900/95 dark:text-berry-50 dark:hover:bg-berry-900"
      >
        <CountdownRing seconds={seconds} progress={progress} />
        <span className="min-w-0 pr-2">
          <span className="flex items-center gap-2 font-display text-sm font-black">
            <RotateCcw className="size-4 text-primary dark:text-blush" />
            Undo tap
          </span>
          <span className="block truncate text-xs font-bold text-berry-700 dark:text-rose-100">
            {beverageName} landed. Tap here to undo.
          </span>
        </span>
      </motion.button>
    </div>
  )
}

function CountdownRing({
  seconds,
  progress,
}: {
  seconds: number
  progress: number
}) {
  return (
    <span className="relative grid size-11 shrink-0 place-items-center">
      <svg className="size-11 -rotate-90" viewBox="0 0 44 44" aria-hidden="true">
        <circle
          cx="22"
          cy="22"
          r="17"
          fill="none"
          stroke="currentColor"
          strokeOpacity="0.16"
          strokeWidth="5"
        />
        <motion.circle
          cx="22"
          cy="22"
          r="17"
          fill="none"
          stroke="currentColor"
          strokeWidth="5"
          strokeLinecap="round"
          pathLength="1"
          strokeDasharray="1"
          animate={{ strokeDashoffset: 1 - progress }}
          transition={{ duration: 0.2, ease: 'linear' }}
          className="text-primary dark:text-blush"
        />
      </svg>
      <span className="absolute font-display text-sm font-black tabular-nums">
        {seconds}
      </span>
    </span>
  )
}

function PrimaryBeverageButton({
  beverage,
  disabled,
  onClick,
  todayCount,
  free = false,
}: {
  beverage: Beverage
  disabled: boolean
  onClick: () => void
  todayCount: number
  free?: boolean
}) {
  const displayName = compactBeverageName(beverage)
  return (
    <motion.button
      type="button"
      disabled={disabled}
      onClick={onClick}
      aria-label={`Log ${beverage.name}`}
      whileTap={{ scale: 0.97 }}
      whileHover={{ y: -3 }}
      className="group flex min-h-48 w-full cursor-pointer flex-col items-stretch overflow-hidden rounded-4xl border-4 border-primary bg-[#fffdf8] text-left text-berry-950 shadow-button ring-2 ring-primary/20 transition hover:bg-white hover:shadow-soft active:translate-y-1 disabled:pointer-events-none disabled:opacity-55 sm:min-h-56 dark:bg-berry-900 dark:text-berry-50 dark:hover:bg-[#202431]"
    >
      <div className="flex grow flex-col justify-between gap-3 p-4 sm:gap-4 sm:p-6">
        <div className="flex items-center justify-between gap-3">
          <span className="text-[0.65rem] font-black uppercase tracking-[0.16em] text-primary dark:text-blush">
            {stockLabelForBeverage(beverage.id)}
          </span>
          <PricePill priceCents={beverage.price_cents} free={free} tone="solid" />
        </div>
        <div className="flex min-w-0 items-end justify-between gap-3">
          <p className="min-w-0 break-words font-display text-xl font-black leading-tight sm:text-4xl">
            {displayName}
          </p>
        </div>
      </div>
      <span className="flex min-h-14 items-center justify-center gap-2 bg-primary px-3 py-3 text-center font-display text-sm font-black uppercase tracking-[0.08em] text-white transition group-hover:bg-secondary dark:bg-blush dark:text-berry-950">
        <MousePointerClick className="size-5 shrink-0" aria-hidden="true" />
        Log drink
      </span>
      <div className="flex items-center justify-between border-t border-primary/25 px-4 py-3 text-xs font-black uppercase tracking-[0.16em] text-berry-700 sm:px-6 dark:border-white/10 dark:text-rose-100">
        <span>Today</span>
        <motion.span
          key={todayCount}
          initial={{ scale: 0.85, opacity: 0.5 }}
          animate={{ scale: 1, opacity: 1 }}
          className="font-display text-3xl tabular-nums text-berry-950 dark:text-berry-50"
        >
          {todayCount}
        </motion.span>
      </div>
    </motion.button>
  )
}

function BeverageButton({
  beverage,
  disabled,
  onClick,
  free = false,
}: {
  beverage: Beverage
  disabled: boolean
  onClick: () => void
  free?: boolean
}) {
  return (
    <motion.button
      type="button"
      disabled={disabled}
      onClick={onClick}
      aria-label={`Log ${beverage.name}`}
      whileTap={{ scale: 0.94 }}
      whileHover={{ y: -3 }}
      className={cn(
        'min-h-32 cursor-pointer rounded-4xl border-2 border-berry-100 bg-[#fffdf8] p-5 text-left text-berry-950 shadow-soft transition hover:border-primary hover:bg-white active:translate-y-1 disabled:pointer-events-none disabled:opacity-55',
        'dark:border-white/10 dark:bg-berry-900 dark:text-berry-50 dark:hover:bg-[#202431]',
        free && 'border-mint',
      )}
    >
      <div className="mb-4 flex items-start justify-between">
        <BeverageIcon
          beverageId={beverage.id}
          className="size-7 text-primary dark:text-blush"
        />
        <PricePill priceCents={beverage.price_cents} free={free} tone="soft" />
      </div>
      <p className="font-display text-2xl font-black">{beverage.name}</p>
      <span className="mt-4 inline-flex min-h-10 items-center gap-2 rounded-full bg-primary px-4 py-2 text-sm font-black text-white dark:bg-blush dark:text-berry-950">
        <CirclePlus className="size-4 shrink-0" aria-hidden="true" />
        Log drink
      </span>
    </motion.button>
  )
}
