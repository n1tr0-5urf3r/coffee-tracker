import { Gift, Sparkles, WashingMachine } from 'lucide-react'
import { useEffect } from 'react'
import { ApiError } from '../api/client'
import { useCleaningStatus, useLogCleaningMutation, useMe } from '../api/queries'
import { formatUnixDate } from '../lib/date'
import { useToast } from '../lib/toast'
import { Button } from './Button'
import { Card } from './Card'

/** Trust-based "I cleaned the coffee machine" button with this week's reward state. */
export function CleaningCard() {
  const { data: status, refetch } = useCleaningStatus()
  const { data: me } = useMe()
  const cleaning = useLogCleaningMutation()
  const { pushToast } = useToast()

  const isGuest = me?.user.user_type === 'guest'
  const blockedUntil = status?.next_allowed_at ?? null
  const lastCleaning = status?.recent[0]

  // Re-check once the personal cooldown is over so the button re-enables.
  useEffect(() => {
    if (blockedUntil === null) return
    const timer = window.setTimeout(
      () => void refetch(),
      Math.max(0, blockedUntil * 1000 - Date.now()) + 1000,
    )
    return () => window.clearTimeout(timer)
  }, [blockedUntil, refetch])

  const logCleaning = () => {
    cleaning.mutate(undefined, {
      onSuccess: (data) => {
        pushToast({
          tone: 'success',
          title: data.rewarded ? 'Sparkling! +1 lootbox' : 'Thanks for cleaning!',
          description: data.rewarded
            ? "You claimed this week's cleaning crate. Open it in your locker."
            : isGuest
              ? 'The machine thanks you. The weekly crate is for members only.'
              : "This week's crate was already claimed, but the machine (and your achievements) thank you.",
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
              ? 'You already logged a cleaning in the last 12 hours.'
              : 'Could not save the cleaning. Try again in a moment.',
        }),
    })
  }

  return (
    <Card className="mt-5 flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
      <div className="flex min-w-0 items-start gap-3">
        <span className="grid size-12 shrink-0 place-items-center rounded-3xl bg-sky/20 text-[#2f6fd6] dark:text-sky">
          <WashingMachine className="size-6" aria-hidden="true" />
        </span>
        <div className="min-w-0">
          <p className="font-display text-lg font-black">Cleaned the machine?</p>
          <p className="text-sm font-bold text-berry-700 dark:text-rose-100">
            {isGuest ? (
              'Thanks for keeping it clean! The weekly crate is for members only.'
            ) : status?.reward_available ? (
              <span className="inline-flex items-center gap-1">
                <Gift className="size-4 text-primary dark:text-blush" aria-hidden="true" />
                This week's cleaning crate is still up for grabs.
              </span>
            ) : status?.reward_claimed_by ? (
              <span className="inline-flex items-center gap-1">
                <Sparkles className="size-4 text-sun" aria-hidden="true" />
                {status.reward_claimed_by} claimed this week's crate. Cleaning still counts!
              </span>
            ) : (
              'Honour system. The first cleaning each week earns a lootbox.'
            )}
          </p>
          {lastCleaning ? (
            <p className="mt-1 text-xs font-bold text-berry-700/80 dark:text-rose-100/70">
              Last cleaned by {lastCleaning.display_name} on {formatUnixDate(lastCleaning.cleaned_at)}
            </p>
          ) : null}
        </div>
      </div>
      <div className="flex shrink-0 flex-col items-stretch gap-1 sm:items-end">
        <Button
          variant="secondary"
          onClick={logCleaning}
          // The server only reports `next_allowed_at` while it is still in the future.
          disabled={cleaning.isPending || blockedUntil !== null}
        >
          <WashingMachine className="size-4" aria-hidden="true" />
          I cleaned it
        </Button>
        {blockedUntil !== null ? (
          <span className="text-center text-xs font-bold text-berry-700 dark:text-rose-100">
            You can log again at{' '}
            {new Date(blockedUntil * 1000).toLocaleTimeString('de-DE', {
              hour: '2-digit',
              minute: '2-digit',
            })}
          </span>
        ) : null}
      </div>
    </Card>
  )
}
