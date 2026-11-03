import { AnimatePresence, motion } from 'framer-motion'
import { Gift, X } from 'lucide-react'
import { type ReactNode, useEffect, useState } from 'react'
import { Link, useLocation } from 'react-router-dom'
import { useLocker, useMe, useWheel } from '../api/queries'
import { isReminderDismissed, reconcileDismissal } from '../lib/reminders'
import { MiniWheel } from './MiniWheel'

const CRATE_DISMISS_KEY = 'coffee-crate-reminder-dismissed'
const WHEEL_DISMISS_KEY = 'coffee-wheel-reminder-dismissed'

function readDismissedCount(key: string) {
  try {
    return Number(sessionStorage.getItem(key) ?? 0)
  } catch {
    return 0
  }
}

function storeDismissedCount(key: string, count: number) {
  try {
    sessionStorage.setItem(key, String(count))
  } catch {
    // per-tab convenience only
  }
}

/**
 * Dismissal per tab: hidden until the count grows past the dismissed one, and
 * reset when the count drops (see `reconcileDismissal`). `count` is undefined
 * while loading.
 */
function useDismissal(key: string, count: number | undefined) {
  const [dismissedCount, setDismissedCount] = useState(() => readDismissedCount(key))
  const reconciled = reconcileDismissal(count, dismissedCount)
  useEffect(() => {
    if (reconciled === dismissedCount) return
    setDismissedCount(reconciled)
    storeDismissedCount(key, reconciled)
  }, [key, reconciled, dismissedCount])
  const dismiss = () => {
    if (count === undefined) return
    setDismissedCount(count)
    storeDismissedCount(key, count)
  }
  return { dismissed: isReminderDismissed(count, reconciled), dismiss }
}

/**
 * Floating reward nudges in the bottom-right corner (above the mobile tab
 * bar): the crate idles, then every few seconds wobbles and hops with a
 * sparkle; next to it a mini lucky wheel turns while free spins are waiting.
 * Each hides on its own page and can be dismissed until more arrive (per
 * tab). Paid spins are never advertised. Holds still under
 * prefers-reduced-motion (see `.crate-wiggle`, `.wheel-idle-spin`).
 */
export function CrateReminder() {
  const { data: me } = useMe()
  const isMember = me?.user.user_type === 'member'
  const { data: locker } = useLocker()
  const { data: wheel } = useWheel(isMember)
  const location = useLocation()

  const crates = locker?.boxes_available ?? 0
  const spins = wheel?.free_spins_available ?? 0
  const crateDismissal = useDismissal(CRATE_DISMISS_KEY, locker?.boxes_available)
  const wheelDismissal = useDismissal(WHEEL_DISMISS_KEY, wheel?.free_spins_available)
  const showCrate = crates > 0 && !crateDismissal.dismissed && location.pathname !== '/locker'
  const showWheel = isMember && spins > 0 && !wheelDismissal.dismissed && location.pathname !== '/wheel'

  return (
    // Above the fixed mobile tab bar (bottom-3, ~4.5rem tall); corner on desktop.
    <div className="pointer-events-none fixed bottom-24 right-3 z-40 flex flex-row-reverse items-end gap-3 sm:bottom-28 md:bottom-6 md:right-6">
      <AnimatePresence>
        {showCrate ? (
          <Bubble
            key="crate"
            to="/locker"
            count={crates}
            label={`${crates} unopened crate${crates === 1 ? '' : 's'}, open the Locker`}
            title={`${crates} crate${crates === 1 ? '' : 's'} waiting`}
            dismissLabel="Hide crate reminder"
            onDismiss={crateDismissal.dismiss}
          >
            <span className="crate-wiggle relative grid size-12 place-items-center rounded-2xl bg-gradient-to-br from-sun to-[#f5a524] text-berry-950 shadow-[0_4px_0_rgb(154_106_12/35%),0_10px_24px_rgb(15_23_42/22%)]">
              <Gift className="size-7" aria-hidden="true" />
              <span aria-hidden="true" className="crate-sparkle absolute -right-1.5 -top-1.5 text-sm">
                ✨
              </span>
            </span>
          </Bubble>
        ) : null}
        {showWheel ? (
          <Bubble
            key="wheel"
            to="/wheel"
            count={spins}
            label={`${spins} free lucky wheel spin${spins === 1 ? '' : 's'}, open the wheel`}
            title={`${spins} free spin${spins === 1 ? '' : 's'} waiting`}
            dismissLabel="Hide lucky wheel reminder"
            onDismiss={wheelDismissal.dismiss}
          >
            <MiniWheel />
          </Bubble>
        ) : null}
      </AnimatePresence>
    </div>
  )
}

function Bubble({
  to,
  count,
  label,
  title,
  dismissLabel,
  onDismiss,
  children,
}: {
  to: string
  count: number
  label: string
  title: string
  dismissLabel: string
  onDismiss: () => void
  children: ReactNode
}) {
  return (
    <motion.div
      layout
      initial={{ opacity: 0, x: 80, scale: 0.9 }}
      animate={{ opacity: 1, x: 0, scale: 1 }}
      exit={{ opacity: 0, x: 80, scale: 0.9 }}
      transition={{ type: 'spring', stiffness: 260, damping: 22 }}
      className="pointer-events-auto relative"
    >
      <Link
        to={to}
        aria-label={label}
        title={title}
        className="block rounded-2xl transition hover:-translate-y-0.5 focus-visible:outline-offset-2"
      >
        <span className="relative grid size-12 place-items-center">
          {children}
          <span className="absolute -bottom-1.5 -right-1.5 grid min-w-5 place-items-center rounded-full bg-primary px-1 text-[0.7rem] font-black text-white ring-2 ring-white dark:ring-berry-900">
            {count > 99 ? '99+' : count}
          </span>
        </span>
      </Link>
      <button
        type="button"
        onClick={onDismiss}
        aria-label={dismissLabel}
        className="absolute -left-2 -top-2 grid size-5 place-items-center rounded-full bg-berry-950/80 text-white shadow-soft transition hover:bg-primary dark:bg-white/90 dark:text-berry-950"
      >
        <X className="size-3" aria-hidden="true" />
      </button>
    </motion.div>
  )
}
