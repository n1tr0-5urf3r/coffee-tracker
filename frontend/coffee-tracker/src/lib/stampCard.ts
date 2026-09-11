export type StampProgress = {
  /** Stamps already on the current card. */
  filled: number
  stampsPerCard: number
  remaining: number
  nextIsFree: boolean
}

/** The drink right after a full card is on the house. */
export function stampCardProgress(card: {
  paid_drink_count: number
  stamps_per_card: number
}): StampProgress {
  const cycle = card.stamps_per_card + 1
  const filled = card.paid_drink_count % cycle
  return {
    filled,
    stampsPerCard: card.stamps_per_card,
    remaining: card.stamps_per_card - filled,
    nextIsFree: filled === card.stamps_per_card,
  }
}
