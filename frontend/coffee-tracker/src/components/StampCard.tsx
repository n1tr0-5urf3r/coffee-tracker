import { Gift } from 'lucide-react'
import type { StampCard as StampCardData } from '../api/client'
import { branding } from '../config/branding'
import { BrandWatermark } from './BrandWatermark'
import { cn } from '../lib/cn'
import { formatEuros } from '../lib/money'
import { EmptySlot, StampImprint } from './StampImprint'

export function StampCard({
  card,
  stampsPerCard,
  monthLabel,
  reducedMotion = false,
  muted = false,
}: {
  card: StampCardData
  stampsPerCard: number
  monthLabel: string
  reducedMotion?: boolean
  muted?: boolean
}) {
  const slots = Array.from({ length: stampsPerCard }, (_, index) => card.stamps[index] ?? null)
  const filled = card.stamps.length
  const rewardReady = card.complete

  return (
    <div
      className={cn(
        'relative mx-auto w-full max-w-3xl overflow-hidden rounded-4xl border-2 border-berry-100 bg-[#fffdf8] p-4 text-berry-950 shadow-soft sm:p-6 dark:border-white/10 dark:bg-berry-900 dark:text-berry-50',
        muted && 'opacity-75',
      )}
    >
      <BrandWatermark className="pointer-events-none absolute -bottom-8 -right-8 w-36 rotate-12 opacity-10" />

      {/* Perforated edge, like a card torn off a pad. */}
      <span
        aria-hidden="true"
        className="pointer-events-none absolute inset-x-0 top-0 h-2 bg-[repeating-radial-gradient(circle_at_6px_0,transparent_0,transparent_4px,var(--color-surface)_4px,var(--color-surface)_7px)] dark:bg-[repeating-radial-gradient(circle_at_6px_0,transparent_0,transparent_4px,var(--color-berry-950)_4px,var(--color-berry-950)_7px)]"
      />

      <div className="rounded-3xl border-2 border-dashed border-berry-100 p-4 sm:p-5 dark:border-white/15">
        <div className="mb-5 flex flex-wrap items-end justify-between gap-3">
          <div>
            <p className="text-[0.65rem] font-black uppercase tracking-[0.22em] text-primary dark:text-blush">
              {branding.organizationShortName} · Coffee fund
            </p>
            <h3 className="font-display text-2xl font-black tracking-tight">
              Stempelkarte {card.index > 1 ? `#${card.index}` : ''}
            </h3>
            <p className="mt-1 text-xs font-bold text-berry-700 dark:text-rose-100">
              {stampsPerCard} stamps = 1 free drink · {monthLabel}
            </p>
          </div>
          <span className="rounded-full bg-berry-100 px-3 py-1 text-xs font-black text-berry-700 dark:bg-white/10 dark:text-berry-50">
            {filled} / {stampsPerCard}
          </span>
        </div>

        <div className="grid grid-cols-4 gap-3 sm:grid-cols-6">
          {slots.map((stamp, index) => (
            <div key={stamp?.consumed_at ?? `empty-${index}`} className="relative aspect-square">
              {stamp ? (
                <StampImprint stamp={stamp} reducedMotion={reducedMotion} />
              ) : (
                <EmptySlot />
              )}
            </div>
          ))}
        </div>

        <div
          className={cn(
            'mt-5 flex items-center gap-4 rounded-3xl border-[3px] border-dashed px-4 py-3',
            rewardReady
              ? 'border-mint bg-mint/10 text-berry-950 dark:text-berry-50'
              : 'border-berry-100 text-berry-700 opacity-70 dark:border-white/15 dark:text-rose-100',
          )}
        >
          <div className="relative size-16 shrink-0">
            {card.reward ? (
              <StampImprint stamp={card.reward} tone="reward" reducedMotion={reducedMotion} />
            ) : (
              <span
                className={cn(
                  'absolute inset-0 flex items-center justify-center rounded-full border-[3px] border-dashed',
                  rewardReady ? 'border-mint text-mint' : 'border-berry-100 text-berry-100 dark:border-white/15 dark:text-white/15',
                )}
              >
                <Gift className="size-7" aria-hidden="true" />
              </span>
            )}
          </div>
          <div className="min-w-0">
            <p className="text-[0.65rem] font-black uppercase tracking-[0.2em]">Free drink</p>
            <p className="text-sm font-bold">
              {card.reward
                ? `Redeemed: ${card.reward.beverage_name} — ${formatEuros(card.reward.price_cents)} saved`
                : rewardReady
                  ? 'Card full! Your next drink is on the house.'
                  : `${stampsPerCard - filled} more stamp${stampsPerCard - filled === 1 ? '' : 's'} until your free drink.`}
            </p>
          </div>
        </div>
      </div>
    </div>
  )
}
