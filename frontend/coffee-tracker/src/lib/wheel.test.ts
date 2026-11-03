import { describe, expect, it } from 'vitest'
import type { WheelOdds } from '../api/client'
import {
  BUTTON_LAUNCH_SPEED,
  MIN_FLICK_SPEED,
  buildSlices,
  launchSpeed,
  normalizeAngle,
  planFinalSpin,
  pointerAngle,
  releaseVelocity,
  sliceIndexAt,
  targetRotation,
  unwrapDelta,
} from './wheel'

const ODDS: WheelOdds[] = [
  { prize: 'nothing', weight: 38, probability: 0.38 },
  { prize: 'oh_no', weight: 20, probability: 0.2 },
  { prize: 'crate_1', weight: 15, probability: 0.15 },
  { prize: 'crate_2', weight: 7, probability: 0.07 },
  { prize: 'crate_3', weight: 3, probability: 0.03 },
  { prize: 'free_drink', weight: 7, probability: 0.07 },
  { prize: 'respin', weight: 10, probability: 0.1 },
]

describe('buildSlices', () => {
  it('covers the full circle with areas matching the odds', () => {
    const slices = buildSlices(ODDS)
    expect(slices[0].start).toBe(0)
    expect(slices.at(-1)!.end).toBeCloseTo(360)
    for (const { prize, probability } of ODDS) {
      const area = slices
        .filter((slice) => slice.prize === prize)
        .reduce((sum, slice) => sum + slice.end - slice.start, 0)
      expect(area).toBeCloseTo(probability * 360)
    }
  })

  it('still places prizes the layout does not know', () => {
    const slices = buildSlices(ODDS, ['nothing'])
    expect(new Set(slices.map((slice) => slice.prize)).size).toBe(ODDS.length)
  })
})

describe('targetRotation', () => {
  it('lands on a slice of the won prize after the requested turns', () => {
    const slices = buildSlices(ODDS)
    let seed = 0.13
    const random = () => {
      seed = (seed * 9301 + 0.49297) % 1
      return seed
    }
    for (const { prize } of ODDS) {
      for (const current of [0, 123.4, -50, 7200.5]) {
        const target = targetRotation(current, slices, prize, { turns: 5, random })
        expect(target - current).toBeGreaterThanOrEqual(5 * 360)
        expect(target - current).toBeLessThan(6 * 360)
        expect(slices[sliceIndexAt(target, slices)].prize).toBe(prize)
      }
    }
  })
})

describe('normalizeAngle', () => {
  it('wraps negative and large angles', () => {
    expect(normalizeAngle(-90)).toBe(270)
    expect(normalizeAngle(725)).toBe(5)
  })
})

describe('flick physics', () => {
  it('measures angles clockwise from the top and ignores the hub', () => {
    expect(pointerAngle(0, -10)).toBeCloseTo(0)
    expect(pointerAngle(10, 0)).toBeCloseTo(90)
    expect(pointerAngle(-10, 0)).toBeCloseTo(-90)
    expect(pointerAngle(1, 1, 5)).toBeNull()
  })

  it('unwraps steps across the ±180° seam', () => {
    expect(unwrapDelta(170, -170)).toBeCloseTo(20)
    expect(unwrapDelta(-170, 170)).toBeCloseTo(-20)
    expect(unwrapDelta(10, 30)).toBeCloseTo(20)
  })

  it('reads the release speed from the last samples, and none after holding still', () => {
    const samples = [
      { t: 0, r: 0 },
      { t: 900, r: 10 },
      { t: 950, r: 40 },
      { t: 1000, r: 80 },
    ]
    expect(releaseVelocity(samples, 1010)).toBeCloseTo(700)
    expect(releaseVelocity(samples, 1200)).toBe(0)
    expect(releaseVelocity([], 0)).toBe(0)
  })

  it('plans a final spin that keeps the launch speed and lands on the prize', () => {
    const slices = buildSlices(ODDS)
    for (const speed of [MIN_FLICK_SPEED, BUTTON_LAUNCH_SPEED, 5000]) {
      const current = 1234.5
      const { target, duration } = planFinalSpin(current, speed, slices, 'free_drink', () => 0.5)
      expect(slices[sliceIndexAt(target, slices)].prize).toBe('free_drink')
      expect(target - current).toBeGreaterThanOrEqual(2 * 360)
      expect(duration).toBeGreaterThanOrEqual(3.5)
      expect(duration).toBeLessThanOrEqual(8)
      // Start speed of the quartic ease-out matches the (clamped) throw.
      const startSpeed = (4 * (target - current)) / duration
      if (duration > 3.5 && duration < 8) expect(startSpeed).toBeCloseTo(launchSpeed(speed))
    }
  })
})
