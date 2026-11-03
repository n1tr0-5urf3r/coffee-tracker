import { Droplets, Gift, Sparkles, Trash2, WashingMachine, type LucideIcon } from 'lucide-react'
import { useEffect } from 'react'
import { ApiError, type CleaningStatus } from '../api/client'
import { useChores, useLogChoreMutation, useMe } from '../api/queries'
import { cn } from '../lib/cn'
import { formatUnixDate } from '../lib/date'
import { useToast } from '../lib/toast'
import { Button } from './Button'
import { Card } from './Card'

const CHORE_ICONS: Record<string, LucideIcon> = {
  clean: WashingMachine,
  grounds: Trash2,
  water: Droplets,
  descale: Sparkles,
}

/** "14:05" today, "Mi. 04:53" on another day (cooldowns can end tomorrow). */
function unlockTime(timestamp: number, now: Date) {
  const date = new Date(timestamp * 1000)
  const sameDay = date.toDateString() === now.toDateString()
  return date.toLocaleString('de-DE', {
    ...(sameDay ? {} : { weekday: 'short' }),
    hour: '2-digit',
    minute: '2-digit',
  })
}

/**
 * Honour-system chores board. For each chore, the first member to do it in a
 * week wins a crate; everyone can still log it (and it counts for achievements).
 */
export function ChoresCard() {
  const { data: board, refetch, dataUpdatedAt } = useChores()
  const { data: me } = useMe()
  const logChore = useLogChoreMutation()
  const { pushToast } = useToast()
  const isGuest = me?.user.user_type === 'guest'
  // "Today" as of the last fetch (keeps rendering pure).
  const renderedAt = new Date(dataUpdatedAt)

  // Re-check once the earliest personal cooldown is over so its button re-enables.
  const nextUnlock = board?.chores
    .map((chore) => chore.next_allowed_at)
    .filter((value): value is number => typeof value === 'number')
    .reduce<number | null>((earliest, value) => (earliest === null ? value : Math.min(earliest, value)), null)
  useEffect(() => {
    if (nextUnlock === null || nextUnlock === undefined) return
    const timer = window.setTimeout(
      () => void refetch(),
      Math.max(0, nextUnlock * 1000 - Date.now()) + 1000,
    )
    return () => window.clearTimeout(timer)
  }, [nextUnlock, refetch])

  const done = (chore: CleaningStatus) => {
    logChore.mutate(chore.chore, {
      onSuccess: (data) => {
        pushToast({
          tone: 'success',
          title: data.rewarded ? `${chore.label}: +1 crate 🎁` : `${chore.label}: thanks!`,
          description: data.rewarded
            ? "You claimed this week's crate for it. Open it in your Locker."
            : isGuest
              ? 'The machine thanks you. Weekly crates are for members only.'
              : "This week's crate was already claimed, but it still counts for your achievements.",
        })
        for (const achievement of data.unlocked_achievements) {
          pushToast({
            tone: 'success',
            title: `Achievement: ${achievement.name}`,
            description: `${achievement.description} +1 lootbox`,
          })
        }
      },
      onError: (error) =>
        pushToast({
          tone: 'error',
          title: 'Not logged',
          description:
            error instanceof ApiError && error.status === 409
              ? 'You already logged this chore in the last 12 hours.'
              : 'Could not save it. Try again in a moment.',
        }),
    })
  }

  return (
    <Card className="mt-5">
      <div className="mb-3 flex flex-wrap items-baseline justify-between gap-2">
        <p className="font-display text-lg font-black">Coffee machine chores</p>
        <p className="text-xs font-bold text-berry-700 dark:text-rose-100">
          {isGuest
            ? 'Thanks for helping! Weekly crates are for members only.'
            : 'Honour system: the first member to do a chore each week wins a crate.'}
        </p>
      </div>
      <ul className="grid gap-2 sm:grid-cols-2">
        {(board?.chores ?? []).map((chore) => {
          const Icon = CHORE_ICONS[chore.chore] ?? WashingMachine
          const last = chore.recent[0]
          const blocked = chore.next_allowed_at !== null && chore.next_allowed_at !== undefined
          return (
            <li
              key={chore.chore}
              className={cn(
                'flex items-center gap-3 rounded-3xl border-2 p-3',
                chore.reward_available && !isGuest
                  ? 'border-sun/70 bg-sun/10'
                  : 'border-berry-100 dark:border-white/10',
              )}
            >
              <span className="grid size-10 shrink-0 place-items-center rounded-2xl bg-sky/20 text-[#2f6fd6] dark:text-sky">
                <Icon className="size-5" aria-hidden="true" />
              </span>
              <div className="min-w-0 grow">
                <p className="font-display text-sm font-black leading-tight">{chore.label}</p>
                <p className="truncate text-xs font-bold text-berry-700 dark:text-rose-100">
                  {chore.reward_available ? (
                    <span className="inline-flex items-center gap-1">
                      <Gift className="size-3 text-primary dark:text-blush" aria-hidden="true" />
                      Crate up for grabs
                    </span>
                  ) : (
                    `Claimed by ${chore.reward_claimed_by ?? 'someone'}`
                  )}
                  {last ? ` · last ${formatUnixDate(last.cleaned_at)}` : ''}
                </p>
              </div>
              <div className="flex shrink-0 flex-col items-end gap-0.5">
                <Button
                  variant="secondary"
                  className="min-h-9 rounded-2xl px-3 py-1.5 text-xs"
                  onClick={() => done(chore)}
                  disabled={logChore.isPending || blocked}
                  aria-label={`I did it: ${chore.label}`}
                >
                  Done
                </Button>
                {blocked ? (
                  <span className="text-[0.65rem] font-bold text-berry-700 dark:text-rose-100">
                    again {unlockTime(chore.next_allowed_at as number, renderedAt)}
                  </span>
                ) : null}
              </div>
            </li>
          )
        })}
      </ul>
    </Card>
  )
}
