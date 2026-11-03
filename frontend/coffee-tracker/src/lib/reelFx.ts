import type { Rarity } from '../api/client'

/**
 * Extra impact effects for the lootbox reel: a low thud when the reel lands
 * and a one-off page shake for legendary drops. Best-effort WebAudio; silent
 * where the browser refuses audio.
 */
let context: AudioContext | null = null

function audio() {
  try {
    context ??= new AudioContext()
    if (context.state === 'suspended') void context.resume()
    return context
  } catch {
    return null
  }
}

const THUD_VOLUME: Record<Rarity, number> = {
  common: 0,
  rare: 0.05,
  epic: 0.08,
  legendary: 0.11,
}

/** Low "boom" under the landing chime, heavier for rarer drops. */
export function playImpactThud(rarity: Rarity) {
  const volume = THUD_VOLUME[rarity]
  const ctx = audio()
  if (!ctx || volume === 0) return
  const at = ctx.currentTime
  const oscillator = ctx.createOscillator()
  const gain = ctx.createGain()
  oscillator.type = 'sine'
  oscillator.frequency.setValueAtTime(95, at)
  oscillator.frequency.exponentialRampToValueAtTime(38, at + 0.35)
  gain.gain.setValueAtTime(0.0001, at)
  gain.gain.exponentialRampToValueAtTime(volume, at + 0.015)
  gain.gain.exponentialRampToValueAtTime(0.0001, at + 0.45)
  oscillator.connect(gain).connect(ctx.destination)
  oscillator.start(at)
  oscillator.stop(at + 0.5)
}

/** Shakes the whole app once (legendary drops only); see LootboxReel.css. */
export function shakeScreen() {
  const root = document.getElementById('root')
  if (!root) return
  root.classList.remove('reel-screen-shake')
  // Force a reflow so the animation restarts if it is already applied.
  void root.offsetWidth
  root.classList.add('reel-screen-shake')
  window.setTimeout(() => root.classList.remove('reel-screen-shake'), 600)
}

/** Restarts a one-shot CSS animation class on an element. */
export function retrigger(element: HTMLElement, className: string) {
  element.classList.remove(className)
  void element.offsetWidth
  element.classList.add(className)
}
