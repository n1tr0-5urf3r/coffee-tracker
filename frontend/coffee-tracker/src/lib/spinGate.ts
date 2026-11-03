import { useSyncExternalStore } from 'react'

/**
 * Set while a lucky wheel spin is in the air. The draw is already stored on
 * the server, so wheel and locker queries hold still until the wheel lands
 * and the page reveals the prize itself.
 */
let spinning = false
const listeners = new Set<() => void>()

export function setSpinInFlight(value: boolean) {
  if (spinning === value) return
  spinning = value
  for (const listener of listeners) listener()
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export function useSpinInFlight() {
  return useSyncExternalStore(subscribe, () => spinning, () => false)
}
