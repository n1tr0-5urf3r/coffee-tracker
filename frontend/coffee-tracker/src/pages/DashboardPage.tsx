import { useState, type ReactNode } from 'react'
import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  Pie,
  PieChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts'
import { Clock3, Flame, Gauge, Trophy, Users, Wallet } from 'lucide-react'
import { Card, Skeleton } from '../components/Card'
import { PageHeader } from '../components/PageHeader'
import {
  useGlobalBeverages,
  useGlobalBuckets,
  useGlobalHourly,
  useMe,
  useMyBeverages,
  useMyHourly,
  useMyPayment,
  useStats,
} from '../api/queries'
import type { GlobalRange } from '../api/client'
import {
  averageFirstCoffeeLabel,
  beverageBreakdownFromEntries,
  caffeineCurve,
  countSince,
  countToday,
  rangeBuckets,
} from '../lib/stats'
import { debtTotals, formatEuros } from '../lib/money'
import { cn } from '../lib/cn'
import { BrandWatermark } from '../components/BrandWatermark'
import { branding } from '../config/branding'

const MEMBER_COLOR = '#A62C4D'
const GUEST_COLOR = '#ffd166'

export function DashboardPage() {
  const { data: me } = useMe()
  const stats = useStats()
  const summary = stats.summary.data
  const buckets = stats.buckets.data
  const hourly = stats.hourly.data
  const caffeine = stats.caffeine.data
  const globalStats = stats.globalStats.data
  const myPayment = useMyPayment()
  const personalDebt = myPayment.data ? debtTotals(myPayment.data) : null

  // Each rhythm card owns its range so the two toggles move independently.
  const [hourlyRange, setHourlyRange] = useState<GlobalRange>('30')
  const [dailyRange, setDailyRange] = useState<GlobalRange>('30')
  const myHourly = useMyHourly(hourlyRange)
  const knHourly = useGlobalHourly(hourlyRange)
  const knDaily = useGlobalBuckets(dailyRange)
  const [beverageRange, setBeverageRange] = useState<GlobalRange>('30')
  const myBeverages = useMyBeverages(beverageRange)
  const knBeverages = useGlobalBeverages(beverageRange)
  const personalDaily = rangeBuckets(buckets?.buckets ?? [], dailyRange)
  const personalBeverages = beverageBreakdownFromEntries(
    myBeverages.data?.beverages ?? [],
  )
  const personalColor = me?.user.user_type === 'guest' ? GUEST_COLOR : MEMBER_COLOR

  const metricCards = [
    ['Today', countToday(buckets), 'Freshly counted'],
    ['Week', countSince(7, buckets), 'Last seven days'],
    ['Month', countSince(30, buckets), 'Rolling month'],
    ['Lifetime', summary?.total_drinks ?? 0, 'All logged drinks'],
  ] as const

  return (
    <div>
      <PageHeader
        eyebrow="Personal dashboard"
        title={`${me?.user.display_name ?? 'Your'} coffee playground`}
      />

      <section className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
        {stats.summary.isLoading
          ? Array.from({ length: 4 }, (_, index) => (
            <Skeleton key={index} className="h-36" />
          ))
          : metricCards.map(([label, value, caption]) => (
            <Card key={label} className="relative overflow-hidden">
              <BrandWatermark
                className="absolute -right-5 -top-8 w-24 rotate-12 opacity-10"
              />
              <p className="text-sm font-black uppercase tracking-[0.16em] text-primary dark:text-blush">
                {label}
              </p>
              <p className="mt-3 font-display text-5xl font-black tabular-nums text-berry-950 dark:text-berry-50">
                {value}
              </p>
              <p className="mt-2 text-sm font-bold text-berry-700 dark:text-rose-100">
                {caption}
              </p>
            </Card>
          ))}
      </section>

      <section className="mt-5 grid gap-5 lg:grid-cols-[1.25fr_0.75fr]">
        <Card className="min-h-80">
          <div className="mb-5 flex items-center justify-between">
            <div>
              <p className="font-display text-xl font-black">Caffeine curve</p>
              <p className="text-sm font-bold text-berry-700 dark:text-rose-100">
                You are currently at {Math.round(caffeine?.caffeine_mg ?? 0)} mg.
              </p>
            </div>
            <Gauge className="size-8 text-primary dark:text-blush" />
          </div>
          <div className="h-56">
            <ResponsiveContainer width="100%" height="100%">
              <AreaChart data={caffeineCurve(caffeine)}>
                <defs>
                  <linearGradient id="caffeineFill" x1="0" x2="0" y1="0" y2="1">
                    <stop offset="0%" stopColor="#991238" stopOpacity={0.75} />
                    <stop offset="100%" stopColor="#D399A9" stopOpacity={0.05} />
                  </linearGradient>
                </defs>
                <XAxis dataKey="label" tick={{ fontSize: 11 }} />
                <YAxis hide />
                <Tooltip content={<DarkTooltip />} />
                <Area
                  type="monotone"
                  dataKey="mg"
                  stroke="#991238"
                  strokeWidth={4}
                  fill="url(#caffeineFill)"
                />
              </AreaChart>
            </ResponsiveContainer>
          </div>
        </Card>

        <Card>
          <div className="mb-5 flex items-center justify-between">
            <div>
              <p className="font-display text-xl font-black">Current streak</p>
              <p className="text-sm font-bold text-berry-700 dark:text-rose-100">
                Longest: {summary?.longest_streak_days ?? 0} weekdays
              </p>
            </div>
            <Flame className="size-9 text-sun" />
          </div>
          <p className="font-display text-7xl font-black text-primary dark:text-blush">
            {summary?.current_streak_days ?? 0}
          </p>
          <p className="mt-2 font-bold">weekdays with a logged drink</p>
          <p className="mt-1 text-sm font-bold text-berry-700 dark:text-rose-100">
            Not including weekends.
          </p>
        </Card>
      </section>

      <section className="mt-5">
        <Card>
          <div className="mb-5 flex flex-wrap items-center gap-3">
            <Wallet className="size-6 text-primary dark:text-blush" />
            <p className="font-display text-xl font-black">Debt overview</p>
          </div>
          <div className="grid gap-5 lg:grid-cols-2">
            <DebtPanel
              title="Your debt"
              loading={myPayment.isLoading || !personalDebt || !myPayment.data}
              openCents={personalDebt?.currentDebtCents ?? 0}
              paidCents={personalDebt?.paidDebtCents ?? 0}
              balanceCents={personalDebt?.balanceCents ?? 0}
              details={
                personalDebt
                  ? [
                      ['Expenses', personalDebt.expenseAmountCents],
                      ['Payments received', personalDebt.receivedPaymentCents],
                    ]
                  : []
              }
              footer={
                myPayment.data
                  ? `${myPayment.data.unpaid_drink_count} unpaid drinks · ${myPayment.data.paid_drink_count} already paid`
                  : null
              }
            />
            <DebtPanel
              title={`${branding.organizationShortName} total`}
              loading={stats.globalStats.isLoading || !globalStats}
              openLabel="Open coffee"
              paidLabel="Recorded paid"
              balanceLabel={`${branding.organizationShortName} balance`}
              openCents={globalStats?.unpaid_total_cents ?? 0}
              paidCents={globalStats?.paid_total_cents ?? 0}
              balanceCents={globalStats?.chair_balance_cents ?? 0}
              details={
                globalStats
                  ? [
                      ['Unpaid coffee debt', globalStats.unpaid_total_cents],
                      ['Staff expenses', globalStats.expense_balance_cents],
                    ]
                  : []
              }
              footer={`Net obligation across ${branding.organizationName} and guests.`}
            />
          </div>
        </Card>
      </section>

      <section className="mt-5">
        <Card>
          <div className="mb-5 flex items-center gap-3">
            <Users className="size-6 text-primary dark:text-blush" />
            <p className="font-display text-xl font-black">{branding.organizationShortName} stats</p>
          </div>
          {stats.globalStats.isLoading || !globalStats ? (
            <Skeleton className="h-40" />
          ) : (
            <div className="grid gap-3 sm:grid-cols-3">
              <InfoPill label="Today" value={String(globalStats.today_drinks)} />
              <InfoPill label="7 days" value={String(globalStats.last_7_days_drinks)} />
              <InfoPill label="Active users" value={String(globalStats.active_users_30d)} />
              <InfoPill
                label={`${branding.organizationShortName} drinks`}
                value={String(globalStats.kn_drinks)}
                caption={`${globalStats.kn_coffees} coffees`}
              />
              <InfoPill
                label="Guest drinks"
                value={String(globalStats.guest_drinks)}
                caption={`${globalStats.guest_coffees} coffees`}
              />
              <InfoPill
                label="All drinks"
                value={String(globalStats.total_drinks)}
                caption={`${globalStats.total_coffees} coffees`}
              />
              <div className="rounded-3xl bg-berry-100 p-4 sm:col-span-3 dark:bg-white/10">
                <p className="text-xs font-black uppercase tracking-[0.16em] text-primary dark:text-blush">
                  Top consumer
                </p>
                <p className="mt-1 font-display text-xl font-black">
                  {globalStats.top_drinker
                    ? `${globalStats.top_drinker.display_name} · ${globalStats.top_drinker.coffee_count} coffees`
                    : 'No opted-in coffees yet'}
                </p>
              </div>
            </div>
          )}
        </Card>
      </section>

      <section className="mt-5 grid gap-5 xl:grid-cols-2">
        <Card className="xl:col-span-2">
          <div className="mb-5 flex flex-wrap items-center justify-between gap-3">
            <div className="flex items-center gap-3">
              <Clock3 className="size-6 text-primary dark:text-blush" />
              <p className="font-display text-xl font-black">Hourly Rhythm</p>
            </div>
            <RangeToggle value={hourlyRange} onChange={setHourlyRange} />
          </div>
          <div className="grid gap-5 lg:grid-cols-2">
            <ComparisonChartBlock
              title="Personal"
              subtitle=""
              loading={myHourly.isLoading || !myHourly.data}
              chart={
                <BarChart data={myHourly.data?.buckets ?? []}>
                  <CartesianGrid stroke="#e2e8f0" strokeDasharray="3 3" vertical={false} />
                  <XAxis dataKey="hour" tick={{ fontSize: 11 }} />
                  <YAxis hide />
                  <Tooltip content={<DarkTooltip />} />
                  <Bar dataKey="count" radius={[10, 10, 4, 4]} fill={personalColor} />
                </BarChart>
              }
            />
            <ComparisonChartBlock
              title={branding.organizationShortName}
              subtitle=""
              loading={knHourly.isLoading || !knHourly.data}
              chart={
                <BarChart data={knHourly.data?.buckets ?? []}>
                  <CartesianGrid stroke="#e2e8f0" strokeDasharray="3 3" vertical={false} />
                  <XAxis dataKey="hour" tick={{ fontSize: 11 }} />
                  <YAxis tick={{ fontSize: 11 }} />
                  <Tooltip content={<DarkTooltip />} />
                  <Bar stackId="kn" dataKey="kn_count" name={branding.organizationShortName} fill={MEMBER_COLOR} radius={[0, 0, 0, 0]} />
                  <Bar stackId="kn" dataKey="guest_count" name="Guests" fill={GUEST_COLOR} radius={[6, 6, 0, 0]} />
                </BarChart>
              }
            />
          </div>
        </Card>

        <Card className="xl:col-span-2">
          <div className="mb-5 flex flex-wrap items-center justify-between gap-3">
            <div className="flex items-center gap-3">
              <Clock3 className="size-6 text-primary dark:text-blush" />
              <p className="font-display text-xl font-black">Daily Rhythm</p>
            </div>
            <RangeToggle value={dailyRange} onChange={setDailyRange} />
          </div>
          <div className="grid gap-5 lg:grid-cols-2">
            <ComparisonChartBlock
              title="Personal"
              subtitle=""
              loading={!buckets}
              chart={
                <BarChart data={personalDaily}>
                  <CartesianGrid stroke="#e2e8f0" strokeDasharray="3 3" vertical={false} />
                  <XAxis
                    dataKey="date"
                    tick={{ fontSize: 10 }}
                    tickFormatter={(value: string) => value.slice(5)}
                    minTickGap={20}
                  />
                  <YAxis tick={{ fontSize: 11 }} />
                  <Tooltip content={<DarkTooltip />} />
                  <Bar dataKey="count" radius={[8, 8, 0, 0]} fill={personalColor} />
                </BarChart>
              }
            />
            <ComparisonChartBlock
              title={branding.organizationShortName}
              subtitle=""
              loading={knDaily.isLoading || !knDaily.data}
              chart={
                <BarChart data={knDaily.data?.buckets ?? []}>
                  <CartesianGrid stroke="#e2e8f0" strokeDasharray="3 3" vertical={false} />
                  <XAxis
                    dataKey="date"
                    tick={{ fontSize: 10 }}
                    tickFormatter={(value: string) => value.slice(5)}
                    minTickGap={20}
                  />
                  <YAxis tick={{ fontSize: 11 }} />
                  <Tooltip content={<DarkTooltip />} />
                  <Bar stackId="kn" dataKey="kn_count" name={branding.organizationShortName} fill={MEMBER_COLOR} />
                  <Bar stackId="kn" dataKey="guest_count" name="Guests" fill={GUEST_COLOR} />
                </BarChart>
              }
            />
          </div>
        </Card>

        <Card>
          <div className="mb-5 flex flex-wrap items-center justify-between gap-3">
            <p className="font-display text-xl font-black">Beverage split</p>
            <RangeToggle value={beverageRange} onChange={setBeverageRange} />
          </div>
          <div className="grid gap-5 lg:grid-cols-2">
            <ComparisonChartBlock
              title="Personal"
              subtitle=""
              loading={myBeverages.isLoading || !myBeverages.data}
              chart={
                <PieChart>
                  <Pie
                    data={personalBeverages}
                    dataKey="value"
                    innerRadius={54}
                    outerRadius={82}
                    paddingAngle={4}
                  >
                    {personalBeverages.map((entry) => (
                      <Cell key={entry.name} fill={entry.fill} />
                    ))}
                  </Pie>
                  <Tooltip content={<DarkTooltip />} />
                </PieChart>
              }
              footer={
                <>
                  <div>{`${myBeverages.data?.beverages.length ?? 0} product types`}</div>
                  <div>{`First busy hour: ${averageFirstCoffeeLabel(hourly)}`}</div>
                </>
              }
            />
            <ComparisonChartBlock
              title={branding.organizationShortName}
              subtitle=""
              loading={knBeverages.isLoading || !knBeverages.data}
              chart={
                <PieChart>
                  <Pie
                    data={beverageBreakdownFromEntries(knBeverages.data?.beverages ?? [])}
                    dataKey="value"
                    innerRadius={54}
                    outerRadius={82}
                    paddingAngle={4}
                  >
                    {beverageBreakdownFromEntries(knBeverages.data?.beverages ?? []).map((entry) => (
                      <Cell key={entry.name} fill={entry.fill} />
                    ))}
                  </Pie>
                  <Tooltip content={<DarkTooltip />} />
                </PieChart>
              }
              footer={`${knBeverages.data?.beverages.length ?? 0} product types`}
            />
          </div>
        </Card>
      </section>

      <Card className="mt-5">
        <div className="mb-5 flex items-center gap-3">
          <Trophy className="size-6 text-sun" />
          <p className="font-display text-xl font-black">Calendar heatmap</p>
        </div>
        <div className="grid grid-cols-14 gap-1 sm:grid-cols-28">
          {(buckets?.buckets.slice(-112) ?? []).map((bucket) => (
            <div
              key={bucket.date}
              title={`${bucket.date}: ${bucket.count}`}
              className={cn('aspect-square rounded-md', heatmapBucketClass(bucket.count))}
            />
          ))}
        </div>
        {buckets?.buckets.length === 0 ? (
          <p className="text-sm font-bold text-berry-700 dark:text-rose-100">
            No squares yet. One tap starts the mosaic.
          </p>
        ) : null}
      </Card>
    </div>
  )
}

function heatmapBucketClass(count: number) {
  if (count > 2) {
    return 'bg-primary dark:bg-blush'
  }
  if (count > 1) {
    return 'bg-blush dark:bg-primary'
  }
  if (count > 0) {
    return 'bg-rose-soft dark:bg-primary/55'
  }
  return 'bg-berry-100/60 ring-1 ring-inset ring-berry-100 dark:bg-white/5 dark:ring-white/10'
}

function InfoPill({
  label,
  value,
  caption,
}: {
  label: string
  value: string
  caption?: string
}) {
  return (
    <div className="rounded-3xl bg-berry-100 p-4 dark:bg-white/10">
      <p className="text-xs font-black uppercase tracking-[0.16em] text-primary dark:text-blush">
        {label}
      </p>
      <p className="mt-1 font-display text-xl font-black">{value}</p>
      {caption ? (
        <p className="mt-1 text-xs font-bold text-berry-700 dark:text-rose-100">
          {caption}
        </p>
      ) : null}
    </div>
  )
}

function ComparisonChartBlock({
  title,
  subtitle,
  loading,
  chart,
  footer,
  accentColor,
}: {
  title: string
  subtitle: string
  loading: boolean
  chart: ReactNode
  footer?: ReactNode
  accentColor?: string
}) {
  return (
    <div className="rounded-3xl bg-berry-100 p-4 dark:bg-white/10">
      <p
        className="font-display text-lg font-black"
        style={accentColor ? { color: accentColor } : undefined}
      >
        {title}
      </p>
      <p className="mt-1 text-sm font-bold text-berry-700 dark:text-rose-100">
        {subtitle}
      </p>
      <div className="mt-4 h-56">
        {loading ? (
          <Skeleton className="h-full w-full" />
        ) : (
          <ResponsiveContainer width="100%" height="100%">
            {chart}
          </ResponsiveContainer>
        )}
      </div>
      {footer ? (
        <div className="mt-3 text-sm font-bold text-berry-700 dark:text-rose-100">
          {footer}
        </div>
      ) : null}
    </div>
  )
}

function DarkTooltip(props: unknown) {
  if (typeof props !== 'object' || props === null) return null
  const p = props as { active?: boolean; payload?: unknown[]; label?: string | number }
  const { active, payload, label } = p
  if (!active || !payload || payload.length === 0) return null

  type Item = { name?: string; value?: number | string; color?: string }

  return (
    <div style={{ zIndex: 9999 }} className="rounded-md border border-white/10 bg-berry-900 p-2 text-sm shadow-lg">
      {label ? <div className="mb-1 text-xs font-bold text-rose-100">{label}</div> : null}
      {payload.map((pItem, idx) => {
        const item = pItem as unknown as Item
        return (
          <div key={idx} className="flex items-center gap-2">
            <span style={{ background: item.color }} className="inline-block h-2 w-2 rounded-full" />
            <span className="text-xs font-bold text-rose-100">{item.name ?? ''}</span>
            <span className="ml-1 tabular-nums text-xs text-berry-50">{item.value}</span>
          </div>
        )
      })}
    </div>
  )
}

function DebtPanel({
  title,
  loading,
  openCents,
  paidCents,
  balanceCents,
  openLabel = 'Open',
  paidLabel = 'Paid',
  balanceLabel = 'Balance',
  details = [],
  footer,
}: {
  title: string
  loading: boolean
  openCents: number
  paidCents: number
  balanceCents: number
  openLabel?: string
  paidLabel?: string
  balanceLabel?: string
  details?: [string, number][]
  footer: string | null
}) {
  const balanceTone =
    balanceCents > 0
      ? 'bg-emerald-100 text-emerald-700 dark:bg-emerald-500/15 dark:text-emerald-200'
      : balanceCents < 0
        ? 'bg-rose-100 text-rose-700 dark:bg-rose-500/15 dark:text-rose-200'
        : 'bg-berry-100 dark:bg-white/10'
  return (
    <div className="rounded-3xl border-2 border-berry-100 p-4 dark:border-white/10">
      <p className="font-display text-lg font-black">{title}</p>
      {loading ? (
        <Skeleton className="mt-3 h-32" />
      ) : (
        <>
          <div className="mt-3 grid gap-3 sm:grid-cols-3">
            <InfoPill label={openLabel} value={formatEuros(openCents)} />
            <InfoPill label={paidLabel} value={formatEuros(paidCents)} />
            <div className={cn('rounded-3xl p-4', balanceTone)}>
              <p className="text-xs font-black uppercase tracking-[0.16em]">
                {balanceLabel}
              </p>
              <p className="mt-1 font-display text-xl font-black">
                {formatEuros(balanceCents)}
              </p>
            </div>
          </div>
          {details.length ? (
            <div className="mt-3 grid gap-2 sm:grid-cols-2">
              {details.map(([label, cents]) => (
                <div
                  key={label}
                  className="flex items-center justify-between gap-3 rounded-2xl bg-berry-100 px-3 py-2 text-sm font-bold dark:bg-white/10"
                >
                  <span className="text-berry-700 dark:text-rose-100">{label}</span>
                  <span className="font-display font-black">{formatEuros(cents)}</span>
                </div>
              ))}
            </div>
          ) : null}
          {footer ? (
            <p className="mt-3 text-sm font-bold text-berry-700 dark:text-rose-100">
              {footer}
            </p>
          ) : null}
        </>
      )}
    </div>
  )
}

function RangeToggle({
  value,
  onChange,
}: {
  value: GlobalRange
  onChange: (next: GlobalRange) => void
}) {
  const options: { value: GlobalRange; label: string }[] = [
    { value: '30', label: '30 days' },
    { value: 'all', label: 'All time' },
  ]
  return (
    <div className="inline-flex rounded-full border-2 border-berry-100 bg-berry-50 p-1 text-xs font-black uppercase tracking-[0.16em] dark:border-white/10 dark:bg-white/5">
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          onClick={() => onChange(option.value)}
          className={cn(
            'rounded-full px-3 py-1 transition-colors',
            option.value === value
              ? 'bg-primary text-white shadow-sm'
              : 'text-berry-700 hover:text-primary dark:text-rose-100',
          )}
        >
          {option.label}
        </button>
      ))}
    </div>
  )
}
