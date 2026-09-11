import { useEffect, useMemo, useRef, useState } from 'react'
import confetti from 'canvas-confetti'
import { ChevronLeft, ChevronRight, Sparkles } from 'lucide-react'
import { useStampCard } from '../api/queries'
import { Skeleton } from '../components/Card'
import { PageHeader } from '../components/PageHeader'
import { StampCard } from '../components/StampCard'
import { StampInkFilter } from '../components/StampImprint'
import { Button } from '../components/Button'
import { formatEuros } from '../lib/money'
import { currentMonthKey, formatMonthLabel, shiftMonth } from '../lib/month'

export function StempelkartePage() {
  const thisMonth = currentMonthKey()
  const [month, setMonth] = useState(thisMonth)
  const { data, isLoading } = useStampCard(month)
  const reducedMotion = useMemo(
    () => window.matchMedia('(prefers-reduced-motion: reduce)').matches,
    [],
  )

  const freeDrinks = data?.free_drinks_earned ?? 0
  // Celebrate a card completed while watching - not the ones already on the
  // card when the page (or another month) is opened.
  const seen = useRef<{ month: string; freeDrinks: number } | null>(null)
  useEffect(() => {
    if (!data) return
    const previous = seen.current
    seen.current = { month, freeDrinks }
    if (!previous || previous.month !== month) return
    if (!reducedMotion && freeDrinks > previous.freeDrinks) {
      void confetti({ particleCount: 90, spread: 70, origin: { y: 0.2 } })
    }
  }, [data, freeDrinks, month, reducedMotion])

  const cards = data?.cards ?? []
  const currentCard = cards.length > 0 ? cards[cards.length - 1] : null
  const earlierCards = cards.slice(0, -1).reverse()
  const monthLabel = formatMonthLabel(month)

  return (
    <div>
      <StampInkFilter />
      <PageHeader eyebrow="Loyalty card" title="Stempelkarte">
        <div className="rounded-full bg-white px-4 py-2 text-sm font-black text-berry-700 shadow-soft dark:bg-berry-900 dark:text-rose-100">
          {currentCard?.stamps.length ?? 0} / {data?.stamps_per_card ?? 12} stamps
        </div>
      </PageHeader>

      <div className="mb-5 flex items-center justify-between gap-3 rounded-4xl border-2 border-berry-100 bg-[#fffdf8] px-3 py-2 shadow-soft dark:border-white/10 dark:bg-berry-900">
        <Button
          variant="ghost"
          onClick={() => setMonth((value) => shiftMonth(value, -1))}
          aria-label="Previous month"
        >
          <ChevronLeft className="size-5" aria-hidden="true" />
        </Button>
        <span className="font-display text-lg font-black text-berry-950 dark:text-berry-50">
          {monthLabel}
        </span>
        <Button
          variant="ghost"
          disabled={month >= thisMonth}
          onClick={() => setMonth((value) => shiftMonth(value, 1))}
          aria-label="Next month"
        >
          <ChevronRight className="size-5" aria-hidden="true" />
        </Button>
      </div>

      {isLoading || !data ? (
        <Skeleton className="h-96" />
      ) : (
        <div className="space-y-5">
          {currentCard ? (
            <StampCard
              card={currentCard}
              stampsPerCard={data.stamps_per_card}
              monthLabel={monthLabel}
              reducedMotion={reducedMotion}
            />
          ) : null}

          <div className="flex flex-wrap items-center gap-3 rounded-4xl border-2 border-berry-100 bg-[#fffdf8] px-5 py-4 text-sm font-bold text-berry-700 shadow-soft dark:border-white/10 dark:bg-berry-900 dark:text-rose-100">
            <Sparkles className="size-5 text-primary dark:text-blush" aria-hidden="true" />
            <span>
              {data.paid_drink_count} paid drinks in {monthLabel} · {freeDrinks} free
              drink{freeDrinks === 1 ? '' : 's'} · {formatEuros(data.saved_cents)} saved
            </span>
          </div>

          {earlierCards.length > 0 ? (
            <div className="space-y-4">
              <p className="text-xs font-black uppercase tracking-[0.16em] text-primary dark:text-blush">
                Full cards this month
              </p>
              {earlierCards.map((card) => (
                <StampCard
                  key={card.index}
                  card={card}
                  stampsPerCard={data.stamps_per_card}
                  monthLabel={monthLabel}
                  reducedMotion
                  muted
                />
              ))}
            </div>
          ) : null}
        </div>
      )}
    </div>
  )
}
