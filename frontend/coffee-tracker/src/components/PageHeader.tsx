import type { ReactNode } from 'react'

export function PageHeader({
  eyebrow,
  title,
  children,
}: {
  eyebrow?: string
  title: string
  children?: ReactNode
}) {
  return (
    <div className="mb-6 flex flex-col gap-4 md:flex-row md:items-end md:justify-between">
      <div>
        {eyebrow ? (
          <p className="mb-2 text-sm font-black uppercase tracking-[0.18em] text-primary dark:text-blush">
            {eyebrow}
          </p>
        ) : null}
        <h1 className="font-display text-4xl font-black tracking-tight text-berry-950 sm:text-5xl dark:text-berry-50">
          {title}
        </h1>
      </div>
      {children}
    </div>
  )
}
