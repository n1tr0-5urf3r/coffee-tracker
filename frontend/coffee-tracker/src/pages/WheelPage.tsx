import confetti from 'canvas-confetti'
import { AnimatePresence, motion } from 'framer-motion'
import { Coffee, Coins, FerrisWheel, Gift, Info, Ticket, Volume2, VolumeX } from 'lucide-react'
import { useQueryClient } from '@tanstack/react-query'
import { useEffect, useMemo, useRef, useState } from 'react'
import { Link } from 'react-router-dom'
import { ApiError, type WheelPrize, type WheelSpinResponse, type WheelStatus } from '../api/client'
import { queryKeys, useMe, useSpinWheelMutation, useWheel, useWheelSpinApply } from '../api/queries'
import { Button } from '../components/Button'
import { Card, Skeleton } from '../components/Card'
import { LuckyWheel, type LuckyWheelHandle } from '../components/LuckyWheel'
import { PageHeader } from '../components/PageHeader'
import { cn } from '../lib/cn'
import { artworkUrl } from '../lib/cosmetics'
import { formatUnixDate } from '../lib/date'
import {
  playWheelLose,
  playWheelWin,
  readSoundPreference,
  writeSoundPreference,
} from '../lib/lootSounds'
import { formatEuros } from '../lib/money'
import { setSpinInFlight } from '../lib/spinGate'
import { useToast } from '../lib/toast'
import { PRIZE_META, buildSlices } from '../lib/wheel'

const BIG_WINS: WheelPrize[] = ['free_drink', 'crate_3']

function newSpinId() {
  // randomUUID only exists in secure contexts (https, localhost).
  if (typeof crypto.randomUUID === 'function') return crypto.randomUUID()
  const bytes = crypto.getRandomValues(new Uint8Array(16))
  bytes[6] = (bytes[6] & 0x0f) | 0x40
  bytes[8] = (bytes[8] & 0x3f) | 0x80
  const hex = [...bytes].map((byte) => byte.toString(16).padStart(2, '0')).join('')
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`
}
const LOSSES: WheelPrize[] = ['nothing', 'oh_no']

const chip =
  'inline-flex items-center gap-2 rounded-full bg-white px-4 py-2 text-sm font-black text-berry-700 shadow-soft dark:bg-berry-900 dark:text-rose-100'

export function WheelPage() {
  const { data: me } = useMe()
  const isMember = me?.user.user_type === 'member'
  const { data: status, isLoading } = useWheel(isMember)

  if (me && !isMember) {
    return (
      <div>
        <PageHeader eyebrow="Glücksrad" title="Lucky wheel" />
        <Card>
          <p className="font-bold text-berry-700 dark:text-rose-100">
            The lucky wheel is for team members. Sign in with your team account to spin.
          </p>
        </Card>
      </div>
    )
  }

  if (isLoading || !status) {
    return (
      <div>
        <PageHeader eyebrow="Glücksrad" title="Lucky wheel" />
        <div className="grid gap-4 lg:grid-cols-[1.4fr_1fr]">
          <Skeleton className="h-[32rem]" />
          <Skeleton className="h-96" />
        </div>
      </div>
    )
  }

  return <Wheel status={status} />
}

function Wheel({ status }: { status: WheelStatus }) {
  const spinMutation = useSpinWheelMutation()
  const applySpin = useWheelSpinApply()
  const queryClient = useQueryClient()
  const { pushToast } = useToast()
  const reducedMotion = useMemo(
    () => window.matchMedia('(prefers-reduced-motion: reduce)').matches,
    [],
  )
  const slices = useMemo(() => buildSlices(status.odds), [status.odds])
  const [sound, setSound] = useState(readSoundPreference)
  const [charging, setCharging] = useState(false)
  const [nudge, setNudge] = useState<'swing' | 'buy' | null>(null)
  /** A paid spin is only thrown after the player bought it with the button. */
  const [armed, setArmed] = useState(false)
  /** Id of the spin being drawn; kept after a lost response so a retry is free. */
  const spinId = useRef<string | null>(null)
  const wheel = useRef<LuckyWheelHandle>(null)
  const [spin, setSpin] = useState<{ prize: WheelPrize; key: number } | null>(null)
  const [result, setResult] = useState<WheelSpinResponse | null>(null)
  const [revealed, setRevealed] = useState(true)

  // Leaving mid-spin still applies the result, so counts stay right.
  const pendingResult = useRef<WheelSpinResponse | null>(null)
  useEffect(
    () => () => {
      setSpinInFlight(false)
      if (pendingResult.current) applySpin(pendingResult.current)
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [],
  )

  const busy = charging || spinMutation.isPending || !revealed
  const free = status.free_spins_available
  const paidLeft = Math.max(status.paid_spins_limit - status.paid_spins_this_week, 0)
  const canSpin = free > 0 || paidLeft > 0
  const ready = free > 0 || (armed && paidLeft > 0)

  const toggleSound = () => {
    setSound(!sound)
    writeSoundPreference(!sound)
  }

  // A weak push nudges the player to swing harder; the hint fades after a moment.
  useEffect(() => {
    if (!nudge) return
    const timer = window.setTimeout(() => setNudge(null), 2200)
    return () => window.clearTimeout(timer)
  }, [nudge])

  /** The wheel was thrown hard enough (or the button pressed): draw the prize. */
  const onLaunch = async () => {
    // Never charge while a free spin is waiting; the server enforces this too.
    const paid = free === 0
    const id = (spinId.current ??= newSpinId())
    setNudge(null)
    setRevealed(false)
    setCharging(true)
    // Hold wheel/locker refetches until the wheel lands, or they'd spoil the prize.
    setSpinInFlight(true)
    void queryClient.cancelQueries({ queryKey: queryKeys.wheel })
    void queryClient.cancelQueries({ queryKey: queryKeys.locker })
    try {
      const data = await spinMutation.mutateAsync({ paid, spinId: id })
      spinId.current = null
      pendingResult.current = data
      setResult(data)
      setSpin((current) => ({ prize: data.spin.prize, key: (current?.key ?? 0) + 1 }))
    } catch (error) {
      setSpinInFlight(false)
      setRevealed(true)
      // A refused spin (4xx) was never booked: forget its id. After a lost
      // response or a server error, a retry reuses it and can't charge twice.
      if (error instanceof ApiError && error.status >= 400 && error.status < 500) {
        spinId.current = null
        setArmed(false)
      }
      if (error instanceof ApiError && error.status === 409) {
        void queryClient.invalidateQueries({ queryKey: queryKeys.wheel })
      }
      pushToast({
        tone: 'error',
        title: 'The wheel is stuck',
        description: error instanceof ApiError ? error.message : 'Try again in a moment.',
      })
    } finally {
      setCharging(false)
    }
  }

  const onLanded = () => {
    setRevealed(true)
    setArmed(false)
    setSpinInFlight(false)
    const data = pendingResult.current
    if (!data) return
    pendingResult.current = null
    applySpin(data)
    const prize = data.spin.prize
    if (sound) {
      if (LOSSES.includes(prize)) playWheelLose()
      else playWheelWin(BIG_WINS.includes(prize))
    }
    if (!reducedMotion) celebrate(prize)
    if (BIG_WINS.includes(prize)) navigator.vibrate?.([40, 60, 40, 60, 120])
    for (const achievement of data.unlocked_achievements) {
      pushToast({
        tone: 'success',
        title: `Achievement: ${achievement.name}`,
        description: `${achievement.description} +1 crate`,
      })
    }
  }

  return (
    <div>
      <PageHeader eyebrow="Glücksrad" title="Lucky wheel">
        <div className="flex flex-wrap gap-2">
          <span className={chip}>
            <Ticket className="size-4 text-primary dark:text-blush" aria-hidden="true" />
            {free} free {free === 1 ? 'spin' : 'spins'}
          </span>
          <span className={chip}>
            {status.paid_spins_this_week}/{status.paid_spins_limit} bought this week
          </span>
          <span className={chip}>{formatEuros(status.price_cents)} per spin</span>
        </div>
      </PageHeader>

      <div className="grid gap-4 lg:grid-cols-[1.4fr_1fr]">
        <Card className="min-w-0 overflow-hidden">
          <div className="mb-4 flex items-start justify-between gap-3">
            <div>
              <p className="font-display text-2xl font-black">Spin to win</p>
              <p className="text-sm font-bold text-berry-700 dark:text-rose-100">
                Crates, free drinks, another go – or Snaggs.
              </p>
            </div>
            <button
              type="button"
              onClick={toggleSound}
              className="grid size-10 shrink-0 place-items-center rounded-full text-berry-700 hover:bg-primary/10 dark:text-rose-100 dark:hover:bg-white/10"
              aria-label={sound ? 'Mute wheel sound' : 'Unmute wheel sound'}
              aria-pressed={sound}
            >
              {sound ? <Volume2 className="size-5" /> : <VolumeX className="size-5" />}
            </button>
          </div>

          <div className="px-2 pt-9 sm:px-8">
            <LuckyWheel
              ref={wheel}
              slices={slices}
              spin={spin}
              pending={charging}
              interactive={ready && !busy}
              reducedMotion={reducedMotion}
              sound={sound}
              onLaunch={() => void onLaunch()}
              onWeakSpin={() => setNudge('swing')}
              onBlockedFlick={() => !busy && canSpin && setNudge('buy')}
              onDone={onLanded}
            />
          </div>

          <div className="mt-5 min-h-32" aria-live="polite">
            <AnimatePresence mode="wait">
              {nudge ? (
                <motion.p
                  key="nudge"
                  initial={{ opacity: 0, y: 6 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0 }}
                  className="py-10 text-center font-display text-lg font-black text-primary dark:text-blush"
                >
                  {nudge === 'buy'
                    ? `Buy a spin first (${formatEuros(status.price_cents)}), then flick it! 🎡`
                    : 'More swing! Flick it clockwise. 💪'}
                </motion.p>
              ) : armed && !busy ? (
                <motion.p
                  key="armed"
                  initial={{ opacity: 0, y: 6 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0 }}
                  className="py-10 text-center font-display text-lg font-black text-berry-950 dark:text-berry-50"
                >
                  Spin bought! Grab the wheel and flick it clockwise. 🎡
                </motion.p>
              ) : result && revealed ? (
                <ResultCard key={spin?.key} result={result} />
              ) : busy ? (
                <motion.p
                  key="spinning"
                  initial={{ opacity: 0, scale: 0.9 }}
                  animate={{ opacity: 1, scale: 1 }}
                  exit={{ opacity: 0 }}
                  className="py-10 text-center font-display text-lg font-black uppercase tracking-[0.2em] text-berry-700 dark:text-rose-100"
                >
                  {charging ? 'Here we go…' : 'Round and round…'}
                </motion.p>
              ) : (
                <motion.p
                  key="idle"
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                  className="py-10 text-center font-bold text-berry-700 dark:text-rose-100"
                >
                  {status.welcome_spin_available
                    ? 'Your first spin is on the house. Grab the wheel and give it a whirl!'
                    : free > 0
                      ? 'You have a free spin waiting. Grab the wheel and flick it!'
                      : paidLeft > 0
                          ? `Out of free spins. Buy one for ${formatEuros(status.price_cents)} (on your tab), then flick the wheel.`
                          : 'That was this week’s last spin. The wheel rests until Monday.'}
                </motion.p>
              )}
            </AnimatePresence>
          </div>

          <div className="mt-2 flex flex-col items-center gap-2">
            {free === 0 && paidLeft > 0 && !armed ? (
              <Button
                onClick={() => {
                  setNudge(null)
                  setArmed(true)
                }}
                disabled={busy}
                className="w-full sm:w-auto sm:min-w-72"
              >
                <Coins className="size-5" aria-hidden="true" />
                Buy a spin · {formatEuros(status.price_cents)}
              </Button>
            ) : (
              <Button
                onClick={() => wheel.current?.launch()}
                disabled={busy || !ready}
                className={cn('w-full sm:w-auto sm:min-w-72', !busy && ready && !reducedMotion && 'loot-pulse')}
              >
                <FerrisWheel className="size-5" aria-hidden="true" />
                {free > 0
                  ? `Free spin${free > 1 ? ` (${free} left)` : ''}`
                  : armed
                    ? 'Spin it for me'
                    : 'Weekly limit reached'}
              </Button>
            )}
            {armed && free === 0 && !busy ? (
              <button
                type="button"
                onClick={() => setArmed(false)}
                className="text-xs font-black text-berry-700 underline-offset-2 hover:underline dark:text-rose-100"
              >
                Changed my mind
              </button>
            ) : null}
            <p className="text-center text-xs font-bold text-berry-700 dark:text-rose-100">
              {free > 0
                ? 'Free spins never cost anything.'
                : armed
                  ? `${formatEuros(status.price_cents)} goes on your tab only once the wheel spins.`
                  : `${paidLeft} of ${status.paid_spins_limit} paid spins left this week.`}
            </p>
          </div>
        </Card>

        <div className="space-y-4">
          {status.free_drinks_available > 0 ? (
            <Card className="border-primary/30 bg-gradient-to-br from-primary to-[#7a1430] text-white dark:border-white/10 dark:from-primary dark:to-[#4a1022]">
              <p className="flex items-center gap-2 font-display text-xl font-black">
                <Coffee className="size-5" aria-hidden="true" />
                {status.free_drinks_available > 1
                  ? `${status.free_drinks_available} free drinks waiting`
                  : 'Your next drink is free'}
              </p>
              <p className="mt-1 text-sm font-bold text-white/85">
                Used automatically the next time you log a drink that costs money.
              </p>
            </Card>
          ) : null}

          <Card>
            <p className="mb-3 font-display text-xl font-black">Odds</p>
            <ul className="space-y-2">
              {status.odds.map((entry) => {
                const meta = PRIZE_META[entry.prize]
                return (
                  <li key={entry.prize} className="flex items-center justify-between gap-3 text-sm font-bold">
                    <span className="flex items-center gap-2">
                      <span
                        className="grid size-7 place-items-center rounded-full text-sm ring-2 ring-berry-950/10"
                        style={{ background: meta.fill }}
                        aria-hidden="true"
                      >
                        {meta.glyph}
                      </span>
                      {meta.label}
                    </span>
                    <span className="tabular-nums text-berry-700 dark:text-rose-100">
                      {Math.round(entry.probability * 100)} %
                    </span>
                  </li>
                )
              })}
            </ul>
            <p className="mt-4 flex gap-2 text-xs font-bold text-berry-700 dark:text-rose-100">
              <Info className="mt-0.5 size-3.5 shrink-0" aria-hidden="true" />
              The house always wins: most of every spin ends up in the Kaffeekasse. At most{' '}
              {status.paid_spins_limit} paid spins per week, booked onto your tab like a drink.
            </p>
          </Card>

          <Card>
            <p className="mb-3 font-display text-xl font-black">Your last spins</p>
            {status.recent_spins.length === 0 ? (
              <p className="text-sm font-bold text-berry-700 dark:text-rose-100">No spins yet.</p>
            ) : (
              <ul className="space-y-2">
                {status.recent_spins.map((entry) => {
                  const meta = PRIZE_META[entry.prize]
                  return (
                    <li key={entry.id} className="flex items-center justify-between gap-3 text-sm font-bold">
                      <span className="flex min-w-0 items-center gap-2">
                        <span aria-hidden="true">{meta.glyph}</span>
                        <span className="truncate">
                          {meta.label}
                          {entry.prize === 'free_drink' ? (entry.redeemed ? ' · used' : ' · waiting') : ''}
                        </span>
                      </span>
                      <span className="shrink-0 tabular-nums text-berry-700 dark:text-rose-100">
                        {entry.cost_cents > 0 ? formatEuros(entry.cost_cents) : 'free'} ·{' '}
                        {formatUnixDate(entry.spun_at)}
                      </span>
                    </li>
                  )
                })}
              </ul>
            )}
          </Card>
        </div>
      </div>
    </div>
  )
}

function ResultCard({ result }: { result: WheelSpinResponse }) {
  const { prize, artwork_path: artwork } = result.spin
  const meta = PRIZE_META[prize]
  const copy: Record<WheelPrize, { title: string; text: string }> = {
    nothing: { title: 'Nothing this time', text: 'The Kaffeekasse thanks you for your generous donation.' },
    oh_no: { title: 'Oh noooo… anyway', text: 'Snaggs came to visit. That is all.' },
    crate_1: { title: '1 crate!', text: 'Waiting for you in the Locker.' },
    crate_2: { title: '2 crates!', text: 'Double the loot, waiting in the Locker.' },
    crate_3: { title: 'JACKPOT – 3 crates!', text: 'The rarest slice on the wheel. Go open them!' },
    free_drink: { title: 'Free drink!', text: 'Your next drink is on the house.' },
    respin: { title: 'Spin again!', text: 'A free spin, on the house.' },
  }

  return (
    <motion.div
      initial={{ opacity: 0, y: 14, scale: 0.94 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, y: -8 }}
      transition={{ type: 'spring', stiffness: 320, damping: 22 }}
      className="flex items-center gap-4 rounded-3xl p-4"
      style={{ background: `${meta.fill}22` }}
    >
      {prize === 'oh_no' && artwork ? (
        <motion.img
          src={artworkUrl(artwork)}
          alt="Snaggs, unimpressed"
          initial={{ rotate: -12, scale: 0.6 }}
          animate={{ rotate: [-12, 8, -4, 0], scale: 1 }}
          transition={{ duration: 0.7 }}
          className="size-24 shrink-0 rounded-3xl object-cover shadow-soft sm:size-28"
        />
      ) : (
        <motion.span
          initial={{ scale: 0.4, rotate: -20 }}
          animate={{ scale: [0.4, 1.25, 1], rotate: 0 }}
          transition={{ duration: 0.6 }}
          className="grid size-16 shrink-0 place-items-center rounded-3xl text-4xl shadow-soft"
          style={{ background: meta.fill }}
          aria-hidden="true"
        >
          {meta.glyph}
        </motion.span>
      )}
      <div className="min-w-0">
        <p className="font-display text-xl font-black">{copy[prize].title}</p>
        <p className="text-sm font-bold text-berry-700 dark:text-rose-100">{copy[prize].text}</p>
        {result.crates_granted > 0 ? (
          <Link
            to="/locker"
            className="mt-2 inline-flex items-center gap-1.5 text-sm font-black text-primary underline-offset-2 hover:underline dark:text-blush"
          >
            <Gift className="size-4" aria-hidden="true" />
            Open in Locker
          </Link>
        ) : null}
      </div>
    </motion.div>
  )
}

const CONFETTI_COLORS = ['#c2284a', '#ffd166', '#f5a524', '#2a9d8f', '#ffffff']

/** Confetti for real prizes; a free drink or the jackpot gets the full show. */
function celebrate(prize: WheelPrize) {
  if (LOSSES.includes(prize)) return
  const burst = (particleCount: number, x: number, spread = 70, y = 0.45) =>
    void confetti({ particleCount, spread, startVelocity: 45, origin: { x, y }, colors: CONFETTI_COLORS })
  if (prize === 'respin' || prize === 'crate_1') return burst(60, 0.5)
  if (prize === 'crate_2') {
    burst(80, 0.35)
    burst(80, 0.65)
    return
  }
  // Free drink and jackpot: cannons from both sides, then a shower.
  const side = (x: number, angle: number) =>
    void confetti({
      particleCount: 90,
      angle,
      spread: 60,
      startVelocity: 60,
      origin: { x, y: 0.7 },
      colors: CONFETTI_COLORS,
    })
  side(0, 60)
  side(1, 120)
  window.setTimeout(() => burst(140, 0.5, 110, 0.35), 300)
  window.setTimeout(() => {
    side(0, 60)
    side(1, 120)
  }, 700)
  if (prize === 'free_drink') {
    window.setTimeout(
      () =>
        void confetti({
          particleCount: 40,
          spread: 160,
          startVelocity: 35,
          origin: { x: 0.5, y: 0.3 },
          shapes: [confetti.shapeFromText({ text: '☕', scalar: 2 })],
          scalar: 2,
        }),
      450,
    )
  }
}
