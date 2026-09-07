import { Coffee } from 'lucide-react'
import { cn } from '../lib/cn'
import { branding } from '../config/branding'

export function LogoMark({
  className,
  label = true,
}: {
  className?: string
  label?: boolean
}) {
  return (
    <div className={cn('flex items-center gap-3', className)}>
      <span className="grid size-12 place-items-center rounded-3xl bg-white shadow-soft ring-2 ring-white/80 dark:bg-berry-900 dark:ring-white/10">
        {branding.logoUrl ? (
          <img src={branding.logoUrl} alt="" className="h-10 w-auto" />
        ) : (
          <Coffee className="size-7 text-primary dark:text-blush" aria-hidden="true" />
        )}
      </span>
      {label ? (
        <span className="font-display text-lg font-black leading-none tracking-tight">
          {branding.appName}
        </span>
      ) : null}
    </div>
  )
}
