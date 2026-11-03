import type { Rarity } from '../api/client'

/**
 * Tiny WebAudio synth for the crate opening: no audio files to ship. Every
 * call is best-effort; browsers that refuse audio simply stay silent.
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

function tone(
  frequency: number,
  {
    start = 0,
    duration = 0.12,
    volume = 0.05,
    type = 'triangle',
    slideTo,
  }: {
    start?: number
    duration?: number
    volume?: number
    type?: OscillatorType
    slideTo?: number
  } = {},
) {
  const ctx = audio()
  if (!ctx) return
  const at = ctx.currentTime + start
  const oscillator = ctx.createOscillator()
  const gain = ctx.createGain()
  oscillator.type = type
  oscillator.frequency.setValueAtTime(frequency, at)
  if (slideTo) oscillator.frequency.exponentialRampToValueAtTime(slideTo, at + duration)
  gain.gain.setValueAtTime(0.0001, at)
  gain.gain.exponentialRampToValueAtTime(volume, at + 0.01)
  gain.gain.exponentialRampToValueAtTime(0.0001, at + duration)
  oscillator.connect(gain).connect(ctx.destination)
  oscillator.start(at)
  oscillator.stop(at + duration + 0.02)
}

/** Click for every tile passing the marker; rises in pitch as the reel slows. */
export function playTick(slowness: number) {
  tone(1100 + slowness * 700 + Math.random() * 60, {
    duration: 0.03,
    volume: 0.03,
    type: 'square',
  })
}

/** Rising whoosh while the crate shakes before the spin. */
export function playCharge() {
  tone(180, { duration: 0.7, volume: 0.04, type: 'sawtooth', slideTo: 900 })
  tone(90, { duration: 0.7, volume: 0.03, type: 'sine', slideTo: 300 })
}

const CHIMES: Record<Rarity, number[]> = {
  common: [784],
  rare: [659, 988],
  epic: [523, 784, 1047],
  legendary: [523, 659, 784, 1047, 1319],
}

/** Landing chime; more notes (and a sparkle) the rarer the drop. */
export function playLanding(rarity: Rarity) {
  tone(110, { duration: 0.25, volume: 0.06, type: 'sine', slideTo: 55 })
  CHIMES[rarity].forEach((note, index) => {
    tone(note, { start: 0.05 + index * 0.08, duration: 0.45, volume: 0.05 })
  })
  if (rarity === 'legendary' || rarity === 'epic') {
    for (let i = 0; i < 6; i += 1) {
      tone(1800 + Math.random() * 1400, {
        start: 0.45 + i * 0.07,
        duration: 0.12,
        volume: 0.018,
        type: 'sine',
      })
    }
  }
}

/** Peg clicking past the wheel's flapper. */
export function playWheelTick() {
  tone(1500 + Math.random() * 120, { duration: 0.022, volume: 0.035, type: 'square' })
  tone(220, { duration: 0.03, volume: 0.02, type: 'sine' })
}

/** The wheel being pulled back before the throw. */
export function playWheelWindup() {
  tone(320, { duration: 0.4, volume: 0.035, type: 'sawtooth', slideTo: 160 })
}

/** Fanfare for a real prize; `big` (jackpot, free drink) adds a second phrase and sparkles. */
export function playWheelWin(big: boolean) {
  const notes = big ? [523, 659, 784, 1047, 784, 1047, 1319] : [659, 784, 1047]
  notes.forEach((note, index) => {
    tone(note, { start: index * 0.11, duration: index === notes.length - 1 ? 0.6 : 0.18, volume: 0.05 })
    tone(note / 2, { start: index * 0.11, duration: 0.16, volume: 0.025, type: 'square' })
  })
  if (big) {
    for (let i = 0; i < 10; i += 1) {
      tone(1800 + Math.random() * 1600, {
        start: 0.7 + i * 0.06,
        duration: 0.12,
        volume: 0.016,
        type: 'sine',
      })
    }
  }
}

/** Sad trombone: wah, wah, wah, waaah. */
export function playWheelLose() {
  const steps: [number, number][] = [
    [392, 370],
    [370, 349],
    [349, 330],
  ]
  steps.forEach(([from, to], index) => {
    tone(from, { start: index * 0.32, duration: 0.3, volume: 0.045, type: 'sawtooth', slideTo: to })
  })
  // The long last note wobbles down.
  for (let i = 0; i < 6; i += 1) {
    tone(i % 2 ? 300 : 311, {
      start: 0.96 + i * 0.12,
      duration: 0.13,
      volume: 0.045,
      type: 'sawtooth',
      slideTo: i === 5 ? 233 : i % 2 ? 311 : 300,
    })
  }
}

/** Shared on/off switch for crate and wheel sounds (per device). */
const SOUND_KEY = 'coffee-loot-sound'

export function readSoundPreference() {
  try {
    return localStorage.getItem(SOUND_KEY) !== 'off'
  } catch {
    return true
  }
}

export function writeSoundPreference(on: boolean) {
  try {
    localStorage.setItem(SOUND_KEY, on ? 'on' : 'off')
  } catch {
    // per-device convenience only
  }
}
