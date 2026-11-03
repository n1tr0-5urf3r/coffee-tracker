import { FerrisWheel } from 'lucide-react'
import { useWheelStats } from '../api/queries'
import type { WheelStats } from '../api/client'
import { formatEuros } from '../lib/money'
import { PRIZE_META } from '../lib/wheel'
import { Card, Skeleton } from './Card'

const percent = (value: number) => `${(value * 100).toFixed(value > 0 && value < 0.1 ? 1 : 0)} %`

/** Staff overview of every lucky wheel draw: money, prizes and weekly activity. */
export function WheelStatsPanel() {
  const { data: stats, isLoading } = useWheelStats()
  if (isLoading || !stats) return <Skeleton className="h-96" />

  return (
    <div className="space-y-4">
      <Card>
        <div className="mb-5 flex items-center gap-3">
          <FerrisWheel className="size-6 text-primary dark:text-blush" aria-hidden="true" />
          <div>
            <p className="font-display text-xl font-black">Lucky wheel</p>
            <p className="text-sm font-bold text-berry-700 dark:text-rose-100">
              Every draw so far. Revenue is booked onto tabs at {formatEuros(stats.price_cents)} per paid spin.
            </p>
          </div>
        </div>
        <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
          <Tile
            label="Net for the Kasse"
            value={formatEuros(stats.net_cents)}
            hint="Revenue minus free drinks used"
            emphasis
          />
          <Tile label="Revenue" value={formatEuros(stats.revenue_cents)} hint={`${stats.paid_spins} paid spins`} />
          <Tile
            label="Free drinks"
            value={`${stats.free_drinks_redeemed}/${stats.free_drinks_won} used`}
            hint={`${formatEuros(stats.free_drinks_value_cents)} value`}
          />
          <Tile label="Crates given" value={String(stats.crates_granted)} hint="Cost nothing" />
          <Tile label="Spins" value={String(stats.total_spins)} hint={`${stats.players} players`} />
          <Tile label="Welcome spins" value={String(stats.welcome_spins)} />
          <Tile label="Re-spins" value={String(stats.voucher_spins)} />
          <Tile
            label="Expected payout"
            value={`${stats.expected_payout_cents.toFixed(1).replace('.', ',')} ct`}
            hint="per paid spin (50 ct drink)"
          />
        </div>
      </Card>

      <div className="grid gap-4 lg:grid-cols-2">
        <PrizeDistribution stats={stats} />
        <WeeklySpins stats={stats} />
      </div>
    </div>
  )
}

function Tile({
  label,
  value,
  hint,
  emphasis = false,
}: {
  label: string
  value: string
  hint?: string
  emphasis?: boolean
}) {
  return (
    <div className="rounded-2xl bg-berry-100/60 p-3 dark:bg-white/5">
      <p className="text-[0.65rem] font-black uppercase tracking-[0.12em] text-berry-700 dark:text-rose-100">
        {label}
      </p>
      <p
        className={
          emphasis
            ? 'mt-1 font-display text-2xl font-black text-primary dark:text-blush'
            : 'mt-1 font-display text-lg font-black'
        }
      >
        {value}
      </p>
      {hint ? <p className="text-xs font-bold text-berry-700 dark:text-rose-100">{hint}</p> : null}
    </div>
  )
}

/** Observed share per prize as a bar, with the promised odds as a tick. */
function PrizeDistribution({ stats }: { stats: WheelStats }) {
  const max = Math.max(...stats.prizes.map((prize) => Math.max(prize.share, prize.expected_share)), 0.01)
  return (
    <Card>
      <p className="font-display text-lg font-black">Prizes drawn</p>
      <p className="mb-4 text-xs font-bold text-berry-700 dark:text-rose-100">
        Bar: share of all draws · tick: promised odds
      </p>
      <ul className="space-y-3">
        {stats.prizes.map((entry) => {
          const meta = PRIZE_META[entry.prize]
          const title = `${meta.label}: ${entry.count} draws, ${percent(entry.share)} (odds ${percent(entry.expected_share)})`
          return (
            <li key={entry.prize} title={title} className="text-sm font-bold">
              <div className="mb-1 flex items-center justify-between gap-3">
                <span className="flex items-center gap-2">
                  <span aria-hidden="true">{meta.glyph}</span>
                  {meta.label}
                </span>
                <span className="tabular-nums text-berry-700 dark:text-rose-100">
                  {entry.count} · {percent(entry.share)}
                </span>
              </div>
              <div className="relative h-2.5 rounded-full bg-berry-100 dark:bg-white/10">
                <div
                  className="h-full rounded-full bg-primary dark:bg-blush"
                  style={{ width: `${(entry.share / max) * 100}%` }}
                />
                <div
                  className="absolute -top-1 h-[1.125rem] w-0.5 rounded-full bg-berry-950 dark:bg-white"
                  style={{ left: `calc(${(entry.expected_share / max) * 100}% - 1px)` }}
                  aria-hidden="true"
                />
              </div>
            </li>
          )
        })}
      </ul>
    </Card>
  )
}

function WeeklySpins({ stats }: { stats: WheelStats }) {
  const max = Math.max(...stats.weeks.map((week) => week.spins), 1)
  return (
    <Card>
      <p className="font-display text-lg font-black">Spins per week</p>
      <p className="mb-4 text-xs font-bold text-berry-700 dark:text-rose-100">
        Last {stats.weeks.length || 0} weeks with spins · revenue on the right
      </p>
      {stats.weeks.length === 0 ? (
        <p className="text-sm font-bold text-berry-700 dark:text-rose-100">Nobody has spun yet.</p>
      ) : (
        <ul className="space-y-2">
          {stats.weeks.map((week) => (
            <li
              key={week.week}
              title={`${week.week}: ${week.spins} spins, ${week.paid_spins} paid, ${formatEuros(week.revenue_cents)}`}
              className="grid grid-cols-[4.5rem_1fr_4.5rem] items-center gap-3 text-sm font-bold"
            >
              <span className="tabular-nums text-berry-700 dark:text-rose-100">{week.week}</span>
              <div className="h-5 rounded-md bg-berry-100 dark:bg-white/10">
                <div
                  className="flex h-full items-center justify-end rounded-md bg-primary px-1.5 text-[0.65rem] font-black text-white dark:bg-blush dark:text-berry-950"
                  style={{ width: `${Math.max((week.spins / max) * 100, 8)}%` }}
                >
                  {week.spins}
                </div>
              </div>
              <span className="text-right tabular-nums">{formatEuros(week.revenue_cents)}</span>
            </li>
          ))}
        </ul>
      )}
    </Card>
  )
}
