import { describe, expect, it } from 'vitest'
import { isReminderDismissed, reconcileDismissal } from './reminders'

describe('reminder dismissal', () => {
  it('stays hidden until more arrive', () => {
    expect(isReminderDismissed(3, 3)).toBe(true)
    expect(isReminderDismissed(4, 3)).toBe(false)
  })

  it('follows the count down so a new item after emptying shows again', () => {
    // Dismissed at 3, all opened (0), then one new crate.
    const afterOpening = reconcileDismissal(0, 3)
    expect(afterOpening).toBe(0)
    expect(isReminderDismissed(1, afterOpening)).toBe(false)
  })

  it('keeps the dismissal while the count is still loading', () => {
    expect(reconcileDismissal(undefined, 3)).toBe(3)
    expect(isReminderDismissed(undefined, 0)).toBe(true)
  })
})
