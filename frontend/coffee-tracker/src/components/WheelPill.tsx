import { Link } from 'react-router-dom'
import { useWheel } from '../api/queries'
import { cn } from '../lib/cn'
import { MiniWheel } from './MiniWheel'

/** Compact lucky-wheel link for the Log page's status row (members only). */
export function WheelPill({ enabled }: { enabled: boolean }) {
  const { data: wheel } = useWheel(enabled)
  if (!enabled || !wheel) return null
  const free = wheel.free_spins_available
  return (
    <Link
      to="/wheel"
      className={cn(
        'flex items-center gap-2 rounded-full py-1 pl-1 pr-4 text-sm font-black shadow-soft transition hover:-translate-y-0.5',
        free > 0
          ? 'bg-sun text-berry-950'
          : 'bg-white text-berry-700 dark:bg-berry-900 dark:text-rose-100',
      )}
    >
      <MiniWheel spinning={free > 0} className="size-8 shadow-none" />
      {free > 0 ? `Lucky wheel · ${free} free spin${free === 1 ? '' : 's'}!` : 'Lucky wheel'}
    </Link>
  )
}
