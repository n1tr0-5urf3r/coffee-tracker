import { describe, expect, it } from 'vitest'
import type { CosmeticItem, RarityInfo } from '../api/client'
import {
  REEL_LENGTH,
  REEL_WINNER_INDEX,
  buildReel,
  describeCrates,
  initials,
  pickWeighted,
  reelOffset,
} from './cosmetics'

function item(id: string, rarity: CosmeticItem['rarity']): CosmeticItem {
  return {
    id,
    rarity,
    slot: 'avatar',
    name: id,
    description: '',
    glyph: null,
    craftable: true,
    craft_cost: 60,
    owned: false,
    equipped: false,
    acquired_at: null,
  }
}

const rarities: RarityInfo[] = [
  { rarity: 'common', label: 'Common', drop_chance_percent: 64, duplicate_beans: 10, craft_cost: 60 },
  { rarity: 'rare', label: 'Rare', drop_chance_percent: 25, duplicate_beans: 25, craft_cost: 150 },
  { rarity: 'epic', label: 'Epic', drop_chance_percent: 9, duplicate_beans: 60, craft_cost: 400 },
  { rarity: 'legendary', label: 'Legendary', drop_chance_percent: 2, duplicate_beans: 150, craft_cost: 1000 },
]

const items = [item('c', 'common'), item('r', 'rare'), item('e', 'epic'), item('l', 'legendary')]

describe('pickWeighted', () => {
  it('maps the roll onto rarity bands', () => {
    expect(pickWeighted(items, rarities, () => 0).id).toBe('c')
    expect(pickWeighted(items, rarities, () => 0.7).id).toBe('r')
    expect(pickWeighted(items, rarities, () => 0.95).id).toBe('e')
    expect(pickWeighted(items, rarities, () => 0.99).id).toBe('l')
  })
})

describe('buildReel', () => {
  it('places the prize at the winner index', () => {
    const winner = item('prize', 'legendary')
    const reel = buildReel(items, rarities, winner, () => 0)
    expect(reel).toHaveLength(REEL_LENGTH)
    expect(reel[REEL_WINNER_INDEX]).toBe(winner)
    expect(reel.filter((tile) => tile.id === 'prize')).toHaveLength(1)
  })

  it('avoids repeating the previous tile', () => {
    let calls = 0
    // Alternates between "always common item" rolls and a rare roll.
    const random = () => (calls++ % 4 < 2 ? 0 : 0.7)
    const reel = buildReel(items, rarities, null, random)
    for (let index = 1; index < reel.length; index += 1) {
      expect(reel[index].id).not.toBe(reel[index - 1].id)
    }
  })

  it('returns an empty reel without a catalog', () => {
    expect(buildReel([], rarities, null)).toEqual([])
  })
})

describe('reelOffset', () => {
  it('centres the target tile in the viewport', () => {
    // Tile 2 spans 240..352; its centre (296) must land at 200 (half of 400).
    expect(reelOffset({ index: 2, tileWidth: 112, gap: 8, viewportWidth: 400, jitter: 0 })).toBe(-96)
    expect(reelOffset({ index: 2, tileWidth: 112, gap: 8, viewportWidth: 400, jitter: 10 })).toBe(-106)
  })
})

describe('initials', () => {
  it('uses first and last name', () => {
    expect(initials('Ada Lovelace')).toBe('AL')
    expect(initials('  cher ')).toBe('CH')
    expect(initials('Jean Claude Van Damme')).toBe('JD')
    expect(initials('')).toBe('?')
  })
})

describe('describeCrates', () => {
  it('groups crate sources into a readable list', () => {
    expect(describeCrates(['daily'])).toBe('Daily crate')
    expect(describeCrates(['daily', 'achievement', 'achievement', 'streak'])).toBe(
      'Daily crate + 2× Achievement + Streak bonus',
    )
    expect(describeCrates(['mystery'])).toBe('Crate')
  })
})
