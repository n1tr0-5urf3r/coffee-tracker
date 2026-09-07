import {
  Bar,
  BarChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts'
import { Crown, Flame, Medal, Sparkles, Trophy, Users } from 'lucide-react'
import { Card, Skeleton } from '../components/Card'
import { PageHeader } from '../components/PageHeader'
import { useCoffeeTwins, useLeaderboard, useMe } from '../api/queries'
import type { CoffeeTwinsResponse, LeaderboardResponse } from '../api/client'
import { cn } from '../lib/cn'
import { formatUnixDate } from '../lib/date'
import { branding } from '../config/branding'

type LeaderboardData = LeaderboardResponse
type CoffeeTwin = CoffeeTwinsResponse['twins'][number]
type AchievementEntry = LeaderboardResponse['achievement_entries'][number]

const CHART_COLORS = {
  current: '#A62C4D',
  longest: '#ffd166',
}

export function LeaderboardPage() {
  const leaderboard = useLeaderboard()
  const twins = useCoffeeTwins()
  const { data: me } = useMe()
  const isOptedIn = me?.user.user_type === 'member' && me.user.leaderboard_opt_in
  const data = leaderboard.data

  return (
    <div>
      <PageHeader eyebrow="Opt-in standings" title={`${branding.organizationShortName} ranking`} />

      {leaderboard.isLoading ? (
        <Skeleton className="h-96" />
      ) : data?.entries.length ? (
        <div className="space-y-4">
          <SummaryStrip data={data} />

          <section className="grid gap-4 xl:grid-cols-[1fr_0.85fr]">
            <CoffeeRanking data={data} />
            <StreakRace data={data} />
          </section>

          <section className="grid gap-4 xl:grid-cols-[1fr_0.85fr]">
            <AchievementStandings data={data} />
            <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-1">
              <RareBadges data={data} />
              <RecentUnlocks data={data} />
            </div>
          </section>

          <CoffeeTwinsPanel
            isOptedIn={Boolean(isOptedIn)}
            twins={twins.data?.twins ?? []}
          />
        </div>
      ) : (
        <Card>
          <p className="font-display text-xl font-black">No opted-in scores yet.</p>
          <p className="mt-2 font-bold text-berry-700 dark:text-rose-100">
            The podium is polished and waiting.
          </p>
        </Card>
      )}
    </div>
  )
}

function SummaryStrip({ data }: { data: LeaderboardData }) {
  const items = [
    [`${branding.organizationShortName} people`, data.summary.visible_participants],
    ['Active streaks', data.summary.active_streaks],
    ['Public unlocks', data.summary.total_public_unlocks],
    ['Active badges', data.summary.total_active_achievements],
  ] as const

  return (
    <section className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
      {items.map(([label, value]) => (
        <div
          key={label}
          className="rounded-3xl bg-berry-100 px-4 py-3 dark:bg-white/10"
        >
          <p className="text-xs font-black uppercase tracking-[0.16em] text-primary dark:text-blush">
            {label}
          </p>
          <p className="font-display text-3xl font-black">{value}</p>
        </div>
      ))}
    </section>
  )
}

function CoffeeRanking({ data }: { data: LeaderboardData }) {
  return (
    <Card>
      <div className="mb-5 flex items-center gap-3">
        <Crown className="size-6 text-primary dark:text-blush" />
        <p className="font-display text-xl font-black">Coffee ranking</p>
      </div>
      <div className="mb-5 grid gap-3 sm:grid-cols-3 sm:items-end">
        {data.entries.slice(0, 3).map((entry, index) => (
          <div
            key={entry.user_id}
            className={cn(
              'rounded-4xl p-4 text-center shadow-soft',
              index === 0 && 'bg-sun text-berry-950 sm:order-2 sm:pb-12',
              index === 1 && 'bg-blush text-berry-950 sm:order-1 sm:pb-7',
              index === 2 && 'bg-mint text-berry-950 sm:order-3 sm:pb-5',
            )}
          >
            <Crown className="mx-auto mb-3 size-7" />
            <p className="font-display text-xl font-black">{entry.display_name}</p>
            <p className="text-sm font-black">{entry.coffee_count} coffees</p>
          </div>
        ))}
      </div>
      <div className="space-y-2">
        {data.entries.slice(3).map((entry) => (
          <div
            key={entry.user_id}
            className="flex flex-wrap items-center justify-between gap-2 rounded-3xl bg-berry-100 px-4 py-3 dark:bg-white/10"
          >
            <span className="min-w-0 break-words font-black">
              #{entry.rank} {entry.display_name}
            </span>
            <span className="font-display font-black">{entry.coffee_count}</span>
          </div>
        ))}
      </div>
    </Card>
  )
}

function StreakRace({ data }: { data: LeaderboardData }) {
  const chartData = data.streak_entries.map((entry) => ({
    name: entry.display_name,
    current: entry.current_streak_days,
    longest: entry.longest_streak_days,
  }))

  // Each row needs enough vertical room for its own tick, otherwise recharts
  // drops every other name to avoid overlap.
  const chartHeight = Math.max(200, chartData.length * 44)

  return (
    <Card>
      <div className="mb-5 flex items-center gap-3">
        <Flame className="size-6 text-primary dark:text-blush" />
        <p className="font-display text-xl font-black">Streak race</p>
      </div>
      {chartData.length ? (
        <>
          <div style={{ height: chartHeight }}>
            <ResponsiveContainer width="100%" height="100%">
              <BarChart data={chartData} layout="vertical" margin={{ left: 12, right: 8 }}>
                <CartesianGrid strokeDasharray="3 3" horizontal={false} />
                <XAxis type="number" allowDecimals={false} />
                <YAxis
                  type="category"
                  dataKey="name"
                  width={160}
                  interval={0}
                  tick={{ fontSize: 13, fontWeight: 700 }}
                  tickFormatter={(value: string) =>
                    value.length > 20 ? `${value.slice(0, 19)}…` : value
                  }
                />
                <Tooltip content={<LeaderboardTooltip />} />
                <Bar dataKey="longest" name="Longest" fill={CHART_COLORS.longest} radius={8} />
                <Bar dataKey="current" name="Current" fill={CHART_COLORS.current} radius={8} />
              </BarChart>
            </ResponsiveContainer>
          </div>
          <p className="mt-3 text-sm font-bold text-berry-700 dark:text-rose-100">
            Current streaks follow weekday rules and skip weekends.
          </p>
        </>
      ) : (
        <EmptyPanel text="No streaks on the board yet." />
      )}
    </Card>
  )
}

function AchievementStandings({ data }: { data: LeaderboardData }) {
  return (
    <Card>
      <div className="mb-5 flex items-center gap-3">
        <Trophy className="size-6 text-primary dark:text-blush" />
        <p className="font-display text-xl font-black">Achievement standings</p>
      </div>
      {data.achievement_entries.length ? (
        <div className="space-y-3">
          {data.achievement_entries.map((entry) => (
            <div
              key={entry.user_id}
              tabIndex={0}
              className="group relative grid gap-2 rounded-3xl bg-berry-100 p-4 outline-none ring-primary/0 transition focus-within:ring-2 hover:bg-berry-50 focus:ring-2 dark:bg-white/10 dark:hover:bg-white/15 sm:grid-cols-[auto_1fr_auto] sm:items-center"
            >
              <span className="font-display text-xl font-black text-primary dark:text-blush">
                #{entry.rank}
              </span>
              <div className="min-w-0">
                <p className="break-words font-display text-lg font-black">
                  {entry.display_name}
                </p>
                <p className="text-sm font-bold text-berry-700 dark:text-rose-100">
                  Latest: {formatDate(entry.latest_unlocked_at)}
                </p>
              </div>
              <span className="font-display text-2xl font-black">
                {entry.unlocked_count}
              </span>
              <AchievementUnlockHover entry={entry} />
            </div>
          ))}
        </div>
      ) : (
        <EmptyPanel text="No public achievement unlocks yet." />
      )}
    </Card>
  )
}

function LeaderboardTooltip(props: unknown) {
  if (typeof props !== 'object' || props === null) return null
  const p = props as { active?: boolean; payload?: unknown[]; label?: string | number }
  const { active, payload, label } = p
  if (!active || !payload || payload.length === 0) return null

  type Item = { name?: string; value?: number | string; color?: string }

  return (
    <div className="rounded-2xl border border-berry-100 bg-[#fffdf8] p-3 text-sm text-berry-950 shadow-soft dark:border-white/10 dark:bg-berry-950 dark:text-berry-50">
      {label ? (
        <div className="mb-2 text-xs font-black text-primary dark:text-blush">
          {label}
        </div>
      ) : null}
      <div className="space-y-1">
        {payload.map((payloadItem, index) => {
          const item = payloadItem as Item
          return (
            <div key={index} className="flex items-center gap-2">
              <span
                style={{ background: item.color }}
                className="inline-block size-2 rounded-full"
              />
              <span className="text-xs font-bold text-berry-700 dark:text-rose-100">
                {item.name ?? ''}
              </span>
              <span className="ml-auto tabular-nums text-xs font-black">
                {item.value}
              </span>
            </div>
          )
        })}
      </div>
    </div>
  )
}

function AchievementUnlockHover({ entry }: { entry: AchievementEntry }) {
  const unlocks = entry.unlocks ?? []

  return (
    <div className="pointer-events-none absolute left-3 right-3 top-[calc(100%-0.25rem)] z-30 hidden rounded-3xl border-2 border-berry-100 bg-[#fffdf8] p-4 text-berry-950 opacity-0 shadow-soft transition group-hover:pointer-events-auto group-hover:block group-hover:opacity-100 group-focus:pointer-events-auto group-focus:block group-focus:opacity-100 dark:border-white/10 dark:bg-berry-950 dark:text-berry-50 md:left-auto md:right-4 md:top-3 md:w-80">
      <div className="mb-3 flex items-center justify-between gap-3">
        <p className="font-display text-lg font-black">Unlocked badges</p>
        <p className="text-sm font-black text-primary dark:text-blush">
          {entry.unlocked_count}
        </p>
      </div>
      {unlocks.length ? (
        <div className="max-h-64 space-y-2 overflow-y-auto pr-1">
          {unlocks.map((unlock) => (
            <div
              key={`${entry.user_id}-${unlock.achievement_id}-${unlock.unlocked_at}`}
              className="rounded-2xl bg-berry-100 px-3 py-2 dark:bg-white/10"
            >
              <p className="break-words font-black">{unlock.name}</p>
              <p className="text-xs font-bold text-berry-700 dark:text-rose-100">
                {formatDate(unlock.unlocked_at)}
              </p>
            </div>
          ))}
        </div>
      ) : (
        <p className="font-bold text-berry-700 dark:text-rose-100">
          No public unlocks listed.
        </p>
      )}
    </div>
  )
}

function RareBadges({ data }: { data: LeaderboardData }) {
  return (
    <Card>
      <div className="mb-5 flex items-center gap-3">
        <Medal className="size-6 text-primary dark:text-blush" />
        <p className="font-display text-xl font-black">Rare badges</p>
      </div>
      {data.rare_achievements.length ? (
        <div className="space-y-2">
          {data.rare_achievements.map((achievement) => (
            <div
              key={achievement.achievement_id}
              className="flex items-center justify-between gap-3 rounded-3xl bg-berry-100 px-4 py-3 dark:bg-white/10"
            >
              <span className="min-w-0 break-words font-black">{achievement.name}</span>
              <span className="font-display font-black">{achievement.unlock_count}</span>
            </div>
          ))}
        </div>
      ) : (
        <EmptyPanel text="No rare badges have public unlocks yet." />
      )}
    </Card>
  )
}

function RecentUnlocks({ data }: { data: LeaderboardData }) {
  return (
    <Card>
      <div className="mb-5 flex items-center gap-3">
        <Sparkles className="size-6 text-primary dark:text-blush" />
        <p className="font-display text-xl font-black">Recent unlocks</p>
      </div>
      {data.recent_unlocks.length ? (
        <div className="space-y-3">
          {data.recent_unlocks.map((unlock) => (
            <div
              key={`${unlock.user_id}-${unlock.achievement_id}-${unlock.unlocked_at}`}
              className="rounded-3xl bg-berry-100 p-4 dark:bg-white/10"
            >
              <p className="font-display text-lg font-black">{unlock.achievement_name}</p>
              <p className="text-sm font-bold text-berry-700 dark:text-rose-100">
                {unlock.display_name} - {formatDate(unlock.unlocked_at)}
              </p>
            </div>
          ))}
        </div>
      ) : (
        <EmptyPanel text="No public unlock activity yet." />
      )}
    </Card>
  )
}

function CoffeeTwinsPanel({
  isOptedIn,
  twins,
}: {
  isOptedIn: boolean
  twins: CoffeeTwin[]
}) {
  return (
    <Card>
      <div className="mb-5 flex items-center gap-3">
        <Users className="size-6 text-primary dark:text-blush" />
        <p className="font-display text-xl font-black">Coffee twins</p>
      </div>
      {!isOptedIn ? (
        <p className="font-bold text-berry-700 dark:text-rose-100">
          Opt into the leaderboard in Settings to discover your rhythm matches.
        </p>
      ) : twins.length ? (
        <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
          {twins.map((twin) => (
            <div
              key={twin.user_id}
              className="rounded-3xl bg-berry-100 p-4 dark:bg-white/10"
            >
              <p className="font-display text-lg font-black">{twin.display_name}</p>
              <p className="text-sm font-bold text-berry-700 dark:text-rose-100">
                {(twin.similarity * 100).toFixed(1)}% similar
              </p>
            </div>
          ))}
        </div>
      ) : (
        <p className="font-bold text-berry-700 dark:text-rose-100">
          No twins yet. Your pattern is still a solo.
        </p>
      )}
    </Card>
  )
}

function EmptyPanel({ text }: { text: string }) {
  return (
    <div className="rounded-3xl bg-berry-100 p-4 font-bold text-berry-700 dark:bg-white/10 dark:text-rose-100">
      {text}
    </div>
  )
}

function formatDate(timestamp: number | null | undefined) {
  return formatUnixDate(timestamp)
}
