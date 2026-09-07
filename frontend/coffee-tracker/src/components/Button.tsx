import type { ButtonHTMLAttributes, ReactNode } from 'react'
import { cn } from '../lib/cn'

type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger'

export function Button({
  className,
  variant = 'primary',
  children,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: ButtonVariant
  children: ReactNode
}) {
  return (
    <button
      className={cn(
        'inline-flex min-h-12 items-center justify-center gap-2 rounded-3xl px-5 py-3 font-display text-sm font-black transition hover:-translate-y-0.5 active:translate-y-1 active:scale-95 disabled:pointer-events-none disabled:opacity-55',
        variant === 'primary' &&
          'bg-primary text-white shadow-button hover:bg-secondary dark:bg-blush dark:text-berry-950',
        variant === 'secondary' &&
          'bg-white text-primary shadow-soft ring-2 ring-primary/10 hover:ring-primary/25 dark:bg-berry-900 dark:text-blush dark:ring-white/10',
        variant === 'ghost' &&
          'bg-transparent text-berry-700 hover:bg-primary/10 dark:text-rose-100 dark:hover:bg-white/10',
        variant === 'danger' &&
          'bg-berry-950 text-white shadow-button hover:bg-primary dark:bg-primary',
        className,
      )}
      {...props}
    >
      {children}
    </button>
  )
}
