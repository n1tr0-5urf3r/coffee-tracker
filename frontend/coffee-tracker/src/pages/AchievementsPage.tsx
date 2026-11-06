import { useEffect, useRef } from 'react'
import confetti from 'canvas-confetti'
import { Gift, Lock, Medal, Sparkles } from 'lucide-react'
import { Link } from 'react-router-dom'
import { motion } from 'motion/react'
import { useAchievements, useLocker } from '../api/queries'
import { Card, Skeleton } from '../components/Card'
import { PageHeader } from '../components/PageHeader'
import { formatUnixDate } from '../lib/date'
import { BrandWatermark } from '../components/BrandWatermark'

export function AchievementsPage() {
  const { data, isLoading } = useAchievements()
  const { data: locker } = useLocker()
  const boxes = locker?.boxes_available ?? 0
  const unlockedCount = data?.achievements.filter((item) => item.unlocked).length ?? 0
  const previousUnlocked = useRef(unlockedCount)

  useEffect(() => {
    const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches
    if (!reduced && unlockedCount > previousUnlocked.current) {
      void confetti({ particleCount: 90, spread: 70, origin: { y: 0.2 } })
    }
    previousUnlocked.current = unlockedCount
  }, [unlockedCount])

  const unlocked = data?.achievements.filter((item) => item.unlocked) ?? []
  const locked = data?.achievements.filter((item) => !item.unlocked) ?? []

  return (
    <div>
      <PageHeader eyebrow="Sticker book" title="Achievements">
        <div className="flex flex-wrap gap-2">
          <div className="rounded-full bg-white px-4 py-2 text-sm font-black text-berry-700 shadow-soft dark:bg-berry-900 dark:text-rose-100">
            {unlockedCount} unlocked
          </div>
          <Link
            to="/locker"
            className="inline-flex items-center gap-2 rounded-full bg-primary px-4 py-2 text-sm font-black text-white shadow-button transition hover:bg-secondary dark:bg-blush dark:text-berry-950"
          >
            <Gift className="size-4" aria-hidden="true" />
            {boxes > 0 ? `${boxes} crate${boxes === 1 ? '' : 's'} to open` : 'Locker'}
          </Link>
        </div>
      </PageHeader>

      {isLoading ? (
        <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
          {Array.from({ length: 6 }, (_, index) => (
            <Skeleton key={index} className="h-52" />
          ))}
        </div>
      ) : (
        <div className="space-y-8">
          <AchievementSection title="Unlocked" items={unlocked} unlocked />
          <AchievementSection title="Still mysterious" items={locked} />
        </div>
      )}
    </div>
  )
}

function AchievementSection({
  title,
  items,
  unlocked = false,
}: {
  title: string
  items: NonNullable<ReturnType<typeof useAchievements>['data']>['achievements']
  unlocked?: boolean
}) {
  if (items.length === 0) {
    return (
      <Card>
        <p className="font-display text-xl font-black">{title}</p>
        <p className="mt-2 font-bold text-berry-700 dark:text-rose-100">
          Nothing in this pile right now.
        </p>
      </Card>
    )
  }

  return (
    <section>
      <h2 className="mb-4 font-display text-2xl font-black">{title}</h2>
      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
        {items.map((achievement) => (
          <motion.div
            key={achievement.id}
            layout
            initial={{ opacity: 0, y: 16 }}
            animate={{ opacity: 1, y: 0 }}
          >
            <Card
              className={
                unlocked
                  ? 'relative min-h-52 overflow-hidden border-sun bg-white dark:bg-berry-900'
                  : 'relative min-h-52 overflow-hidden grayscale'
              }
            >
              <BrandWatermark
                className="absolute -right-8 -top-8 w-32 rotate-12 opacity-15"
              />
              <div className="relative">
                <div className="mb-7 inline-grid size-16 place-items-center rounded-3xl bg-primary text-white shadow-button dark:bg-blush dark:text-berry-950">
                  {unlocked ? <Medal className="size-8" /> : <Lock className="size-8" />}
                </div>
                <p className="font-display text-2xl font-black">{achievement.name}</p>
                <p className="mt-2 text-sm font-bold text-berry-700 dark:text-rose-100">
                  {achievement.description ?? achievement.hint}
                </p>
                {achievement.progress ? <ProgressBar {...achievement.progress} /> : null}
                <p className="mt-3 inline-flex items-center gap-1 text-xs font-black text-primary dark:text-blush">
                  <Gift className="size-3" aria-hidden="true" /> Reward: 1 lootbox
                </p>
                {achievement.unlocked_at ? (
                  <p className="mt-4 inline-flex items-center gap-2 rounded-full bg-sun/45 px-3 py-1 text-xs font-black">
                    <Sparkles className="size-3" />
                    {formatUnixDate(achievement.unlocked_at)}
                  </p>
                ) : null}
              </div>
            </Card>
          </motion.div>
        ))}
      </div>
    </section>
  )
}

/** How far a locked achievement is, e.g. 3 / 10 espressos. */
function ProgressBar({ current, target }: { current: number; target: number }) {
  const percent = Math.round((Math.min(current, target) / target) * 100)
  return (
    <div className="mt-3">
      <div className="mb-1 flex justify-between text-xs font-black text-berry-700 dark:text-rose-100">
        <span>Progress</span>
        <span className="tabular-nums">
          {current} / {target}
        </span>
      </div>
      <div
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={target}
        aria-valuenow={current}
        aria-label={`${current} of ${target}`}
        className="h-2.5 overflow-hidden rounded-full bg-primary/15 dark:bg-white/15"
      >
        <div
          className="h-full rounded-full bg-primary transition-[width] duration-500 dark:bg-blush"
          style={{ width: `${percent}%` }}
        />
      </div>
    </div>
  )
}
