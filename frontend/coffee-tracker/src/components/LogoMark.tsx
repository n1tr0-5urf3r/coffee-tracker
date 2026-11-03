import { Coffee } from 'lucide-react'
import { cn } from '../lib/cn'
import { branding } from '../config/branding'
import { useSeasonalTheme } from '../lib/seasonalTheme'
import { SEASONS } from '../lib/seasons'

export function LogoMark({
  className,
  label = true,
}: {
  className?: string
  label?: boolean
}) {
  const { activeSeason } = useSeasonalTheme()
  return (
    <div className={cn('flex items-center gap-3', className)}>
      <span className="relative grid size-12 place-items-center rounded-3xl bg-white shadow-soft ring-2 ring-white/80 dark:bg-berry-900 dark:ring-white/10">
        {branding.logoUrl ? (
          <img src={branding.logoUrl} alt="" className="h-10 w-auto" />
        ) : (
          <Coffee className="size-7 text-primary dark:text-blush" aria-hidden="true" />
        )}
        {activeSeason ? (
          <span
            aria-hidden="true"
            className="absolute -right-2 -top-2 rotate-12 text-lg leading-none drop-shadow-sm"
          >
            {SEASONS[activeSeason].emoji}
          </span>
        ) : null}
      </span>
      {label ? (
        <span className="font-display text-lg font-black leading-none tracking-tight">
          {branding.appName}
        </span>
      ) : null}
    </div>
  )
}
