import { cn } from '../lib/cn'
import { PRIZE_META, SLICE_LAYOUT } from '../lib/wheel'

const STEP = 360 / SLICE_LAYOUT.length
const STOPS = SLICE_LAYOUT.map(
  (prize, index) => `${PRIZE_META[prize].fill} ${index * STEP}deg ${(index + 1) * STEP}deg`,
).join(', ')

/**
 * Tiny version of the lucky wheel: equal slices in the same colours. With
 * `spinning` it does a lazy idle turn (still under prefers-reduced-motion).
 */
export function MiniWheel({ spinning = true, className }: { spinning?: boolean; className?: string }) {
  return (
    <span
      className={cn(
        'relative grid size-12 shrink-0 place-items-center rounded-full bg-[#4a1022] shadow-[0_4px_0_rgb(74_16_34/35%),0_10px_24px_rgb(15_23_42/22%)]',
        className,
      )}
      aria-hidden="true"
    >
      <span
        className={cn('size-[83%] rounded-full', spinning && 'wheel-idle-spin')}
        style={{ background: `conic-gradient(${STOPS})` }}
      />
      <span className="absolute size-[25%] rounded-full border-2 border-[#ffd166] bg-primary" />
      <span className="absolute -top-1 left-1/2 size-0 -translate-x-1/2 border-x-[5px] border-t-[8px] border-x-transparent border-t-white drop-shadow" />
    </span>
  )
}
