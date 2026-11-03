/**
 * Dismissal of a count-based reminder (crates, free spins): hidden while the
 * count is at or below the count it was dismissed at; it comes back once more
 * arrive. When the count drops (crates opened, spins used), the dismissal
 * follows it down, so a new crate after emptying the stack shows again.
 * `count` is undefined while the data is still loading.
 */
export function reconcileDismissal(count: number | undefined, dismissedAt: number) {
  if (count === undefined) return dismissedAt
  return Math.min(count, dismissedAt)
}

export function isReminderDismissed(count: number | undefined, dismissedAt: number) {
  return count === undefined || count <= dismissedAt
}
