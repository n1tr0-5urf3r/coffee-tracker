import { describe, expect, it } from 'vitest'
import { stampJitter } from './stampJitter'

describe('stampJitter', () => {
  it('is stable for the same seed', () => {
    expect(stampJitter(1_700_000_000)).toEqual(stampJitter(1_700_000_000))
  })

  it('differs between neighbouring stamps', () => {
    expect(stampJitter(10).rotation).not.toBe(stampJitter(11).rotation)
  })

  it('stays within the plausible range of a hand-pressed stamp', () => {
    for (let seed = 0; seed < 200; seed += 1) {
      const jitter = stampJitter(seed * 37)
      expect(Math.abs(jitter.rotation)).toBeLessThanOrEqual(14)
      expect(Math.abs(jitter.offsetX)).toBeLessThanOrEqual(2)
      expect(Math.abs(jitter.offsetY)).toBeLessThanOrEqual(2)
      expect(jitter.scale).toBeGreaterThanOrEqual(0.94)
      expect(jitter.scale).toBeLessThanOrEqual(1.04)
      expect(jitter.inkOpacity).toBeGreaterThanOrEqual(0.72)
      expect(jitter.inkOpacity).toBeLessThanOrEqual(0.96)
    }
  })
})
