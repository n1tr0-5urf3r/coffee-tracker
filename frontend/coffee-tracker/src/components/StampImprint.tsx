import { motion } from 'framer-motion'
import { Gift, Stamp as StampIcon } from 'lucide-react'
import type { Stamp } from '../api/client'
import { BeverageIcon, stampLabel } from '../lib/beverageIcons'
import { stampJitter } from '../lib/stampJitter'
import { cn } from '../lib/cn'

/**
 * Rough-edged ink filter shared by every imprint on the page. Rendered once,
 * referenced through `filter: url(#stamp-ink)`.
 */
export function StampInkFilter() {
  return (
    <svg aria-hidden="true" className="pointer-events-none absolute size-0">
      <filter id="stamp-ink">
        <feTurbulence type="fractalNoise" baseFrequency="0.9" numOctaves="2" result="noise" />
        <feDisplacementMap in="SourceGraphic" in2="noise" scale="1.6" />
      </filter>
    </svg>
  )
}

function shortDate(consumedAt: number) {
  return new Date(consumedAt * 1000).toLocaleDateString('en-GB', {
    day: '2-digit',
    month: 'short',
  })
}

export function StampImprint({
  stamp,
  tone = 'ink',
  reducedMotion = false,
}: {
  stamp: Stamp
  tone?: 'ink' | 'reward'
  reducedMotion?: boolean
}) {
  const label = tone === 'reward' ? 'Free' : stampLabel(stamp.beverage_id, stamp.beverage_name)
  const jitter = stampJitter(stamp.consumed_at + stamp.position)

  return (
    <motion.div
      // The imprint starts visible: even if the press animation never runs
      // (reduced motion, a screenshot mid-flight) the stamp must be there.
      initial={reducedMotion ? false : { scale: 1.15, opacity: 0.45 }}
      animate={{ scale: jitter.scale, opacity: jitter.inkOpacity }}
      transition={{ type: 'spring', stiffness: 420, damping: 18 }}
      style={{ rotate: jitter.rotation, x: jitter.offsetX, y: jitter.offsetY }}
      title={`${stamp.beverage_name} · ${new Date(stamp.consumed_at * 1000).toLocaleString('en-GB')}`}
      className={cn(
        'absolute inset-0 flex flex-col items-center justify-center rounded-full border-[3px] mix-blend-multiply dark:mix-blend-screen',
        tone === 'reward'
          ? 'border-mint text-mint'
          : 'border-primary text-primary dark:border-blush dark:text-blush',
      )}
    >
      <span
        className="pointer-events-none absolute inset-[5px] rounded-full border border-current opacity-70"
        style={{ filter: 'url(#stamp-ink)' }}
        aria-hidden="true"
      />
      <span className="text-[0.45rem] font-black uppercase leading-none tracking-[0.08em] opacity-80 sm:text-[0.5rem]">
        {shortDate(stamp.consumed_at)}
      </span>
      {tone === 'reward' ? (
        <Gift className="my-[3px] size-5 stroke-[2.4] sm:size-6" aria-hidden="true" />
      ) : (
        <BeverageIcon
          beverageId={stamp.beverage_id}
          className="my-[3px] size-6 stroke-[2.4]"
        />
      )}
      <span className="max-w-[90%] truncate text-[0.45rem] font-black uppercase leading-none tracking-[0.08em] sm:text-[0.5rem] sm:tracking-[0.12em]">
        {label}
      </span>
    </motion.div>
  )
}

export function EmptySlot() {
  return (
    <span className="absolute inset-0 flex items-center justify-center rounded-full border-[3px] border-dashed border-berry-100 dark:border-white/15">
      <StampIcon
        className="size-6 text-berry-100 dark:text-white/15"
        aria-hidden="true"
      />
    </span>
  )
}
