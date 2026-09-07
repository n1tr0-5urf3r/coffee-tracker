import type { HTMLAttributes, ReactNode } from 'react'
import { cn } from '../lib/cn'

export function Card({
  className,
  children,
  ...props
}: HTMLAttributes<HTMLDivElement> & { children: ReactNode }) {
  return (
    <div
      className={cn(
        'rounded-4xl border-2 border-berry-100 bg-[#fffdf8] p-5 text-berry-950 shadow-soft dark:border-white/10 dark:bg-berry-900 dark:text-berry-50',
        className,
      )}
      {...props}
    >
      {children}
    </div>
  )
}

export function Skeleton({ className }: { className?: string }) {
  return (
    <div
      className={cn(
        'animate-pulse rounded-3xl bg-primary/10 dark:bg-white/10',
        className,
      )}
    />
  )
}
