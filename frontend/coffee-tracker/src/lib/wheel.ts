import type { WheelOdds, WheelPrize } from '../api/client'

/** Display data for every wheel prize. */
export const PRIZE_META: Record<
  WheelPrize,
  { label: string; short: string; glyph: string; fill: string; ink: string }
> = {
  nothing: { label: 'Nothing', short: 'Nope', glyph: '💨', fill: '#fff4f6', ink: '#7a2440' },
  oh_no: { label: 'Oh noooo… anyway', short: 'Oh no', glyph: '🙈', fill: '#7c3a52', ink: '#ffffff' },
  crate_1: { label: '1 crate', short: '1 crate', glyph: '🎁', fill: '#ffd166', ink: '#4a1022' },
  crate_2: { label: '2 crates', short: '2 crates', glyph: '🎁', fill: '#f5a524', ink: '#4a1022' },
  crate_3: { label: '3 crates', short: '3×', glyph: '💎', fill: '#7c3aed', ink: '#ffffff' },
  free_drink: { label: 'Free drink', short: 'Free', glyph: '☕', fill: '#c2284a', ink: '#ffffff' },
  respin: { label: 'Spin again', short: 'Again', glyph: '🔁', fill: '#2a9d8f', ink: '#ffffff' },
}

/** From this many paid spins in a week, the wheel page shows a gambling warning. */
export const GAMBLING_WARNING_PAID_SPINS = 3

/** Whether the player bought enough spins this week to see the gambling warning. */
export function showGamblingWarning(paidSpinsThisWeek: number) {
  return paidSpinsThisWeek >= GAMBLING_WARNING_PAID_SPINS
}

/**
 * Order of the slices around the wheel. Frequent prizes are split into several
 * slices so the wheel looks lively; every slice of a prize gets an equal share
 * of that prize's weight, so the area on the wheel matches the real odds.
 */
export const SLICE_LAYOUT: WheelPrize[] = [
  'nothing',
  'crate_1',
  'oh_no',
  'free_drink',
  'nothing',
  'crate_2',
  'respin',
  'oh_no',
  'nothing',
  'crate_1',
  'crate_3',
]

export type WheelSlice = {
  prize: WheelPrize
  /** Degrees clockwise from the top (the pointer), `start < end`. */
  start: number
  end: number
}

export function buildSlices(odds: WheelOdds[], layout: WheelPrize[] = SLICE_LAYOUT): WheelSlice[] {
  const weights = new Map(odds.map((entry) => [entry.prize, entry.weight]))
  const parts = new Map<WheelPrize, number>()
  for (const prize of layout) parts.set(prize, (parts.get(prize) ?? 0) + 1)
  // Prizes missing from the layout still need room on the wheel.
  const fullLayout = [...layout, ...odds.map((entry) => entry.prize).filter((prize) => !parts.has(prize))]
  for (const prize of fullLayout) if (!parts.has(prize)) parts.set(prize, 1)

  const sizes = fullLayout.map((prize) => (weights.get(prize) ?? 0) / (parts.get(prize) ?? 1))
  const total = sizes.reduce((sum, size) => sum + size, 0) || 1
  let angle = 0
  return fullLayout
    .map((prize, index) => {
      const width = (sizes[index] / total) * 360
      const slice = { prize, start: angle, end: angle + width }
      angle += width
      return slice
    })
    .filter((slice) => slice.end > slice.start)
}

export function normalizeAngle(angle: number) {
  return ((angle % 360) + 360) % 360
}

/** Index of the slice under the pointer when the wheel is turned by `rotation` degrees. */
export function sliceIndexAt(rotation: number, slices: WheelSlice[]) {
  const angle = normalizeAngle(-rotation)
  const index = slices.findIndex((slice) => angle >= slice.start && angle < slice.end)
  return index === -1 ? slices.length - 1 : index
}

/**
 * Final rotation that lands a random slice of `prize` under the pointer after
 * at least `turns` full turns from `current`. `random` picks the slice and
 * the spot inside it (kept away from the edges so the result is unambiguous).
 */
export function targetRotation(
  current: number,
  slices: WheelSlice[],
  prize: WheelPrize,
  { turns = 6, random = Math.random }: { turns?: number; random?: () => number } = {},
) {
  const candidates = slices.filter((slice) => slice.prize === prize)
  const slice = candidates[Math.floor(random() * candidates.length)] ?? slices[0]
  const width = slice.end - slice.start
  const angle = slice.start + width * (0.18 + random() * 0.64)
  // A point at `angle` sits under the pointer when rotation ≡ -angle (mod 360).
  const delta = normalizeAngle(-angle - current)
  return current + turns * 360 + delta
}

/* ---------- Grab-and-flick physics (see components/LuckyWheel.tsx) ---------- */

/** A clockwise flick at least this fast (deg/s) launches a spin. */
export const MIN_FLICK_SPEED = 250
/** Launch speeds are clamped into this range (deg/s). */
export const MIN_LAUNCH_SPEED = 700
export const MAX_LAUNCH_SPEED = 2000
/** Speed the spin button throws the wheel with (deg/s). */
export const BUTTON_LAUNCH_SPEED = 1300

const clamp = (value: number, min: number, max: number) => Math.min(Math.max(value, min), max)

/** Final spin ease: the derivative at 0 is 4, so the wheel keeps its launch speed. */
export const easeOutQuart = (t: number) => 1 - (1 - t) ** 4

/**
 * Angle of a pointer around the wheel centre, in degrees clockwise from the
 * top (the pointer), or `null` too close to the hub to be meaningful.
 */
export function pointerAngle(dx: number, dy: number, minRadius = 0) {
  if (Math.hypot(dx, dy) < minRadius) return null
  return (Math.atan2(dx, -dy) * 180) / Math.PI
}

/** Smallest signed step from angle `from` to angle `to` (-180..180]. */
export function unwrapDelta(from: number, to: number) {
  const delta = normalizeAngle(to - from)
  return delta > 180 ? delta - 360 : delta
}

export type RotationSample = { t: number; r: number }

/**
 * Angular velocity (deg/s) at release from recent drag samples (`t` in ms).
 * Holding still before letting go means no throw.
 */
export function releaseVelocity(samples: RotationSample[], releasedAt: number, windowMs = 100) {
  const last = samples.at(-1)
  if (!last || releasedAt - last.t > 80) return 0
  const recent = samples.filter((sample) => last.t - sample.t <= windowMs)
  const first = recent[0]
  const elapsed = (last.t - first.t) / 1000
  return elapsed > 0 ? (last.r - first.r) / elapsed : 0
}

export function launchSpeed(velocity: number) {
  return clamp(velocity, MIN_LAUNCH_SPEED, MAX_LAUNCH_SPEED)
}

/**
 * Plans the final run onto `prize` from the current rotation, starting at
 * `speed` deg/s and slowing down with `easeOutQuart` (start slope 4), so the
 * hand-over from a flick or the cruise is seamless. Faster throws travel
 * further but always take a few seconds.
 */
export function planFinalSpin(
  current: number,
  speed: number,
  slices: WheelSlice[],
  prize: WheelPrize,
  random: () => number = Math.random,
) {
  const v = launchSpeed(speed)
  // Distance a ~7 s quartic ease-out covers at this start speed; one turn is
  // left to targetRotation's alignment.
  const turns = Math.max(2, Math.round((v * 7) / 4 / 360) - 1)
  const target = targetRotation(current, slices, prize, { turns, random })
  const duration = clamp((4 * (target - current)) / v, 3.5, 8)
  return { target, duration }
}
