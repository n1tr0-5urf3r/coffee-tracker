import { apiUrl, type CosmeticItem, type CosmeticSlot, type LockerResponse, type Rarity, type RarityInfo } from '../api/client'

/** Rarity colours, loosely following the classic weapon-case tiers. */
export const RARITY_COLORS: Record<Rarity, string> = {
  common: '#4b69ff',
  rare: '#8847ff',
  epic: '#d32ce6',
  legendary: '#e4ae39',
}

export const RARITY_ORDER: Rarity[] = ['common', 'rare', 'epic', 'legendary']

export const SLOT_LABELS: Record<CosmeticSlot, string> = {
  avatar: 'Avatars',
  frame: 'Frames',
  title: 'Titles',
  cup: 'Cups',
  stamp: 'Stamps',
  skin: 'UI skins',
  collectible: 'Collectibles',
}

export const SLOT_ORDER: CosmeticSlot[] = [
  'avatar',
  'frame',
  'title',
  'cup',
  'stamp',
  'skin',
  'collectible',
]

/**
 * URL of an item's artwork as returned by the API. Public avatars are static
 * files (`cosmetics/avatars/…`); private ones (team photos, stickers) are
 * served by the backend to members only (`api/v1/cosmetics/artwork/…`).
 */
export function artworkUrl(path: string) {
  return path.startsWith('api/') ? apiUrl(path) : `${import.meta.env.BASE_URL}${path}`
}

/** Artwork path of the avatar a locker has equipped. */
export function equippedAvatarArtwork(locker: LockerResponse | undefined) {
  if (!locker?.equipped.avatar) return null
  return locker.items.find((item) => item.id === locker.equipped.avatar)?.artwork_path ?? null
}

/** Tailwind classes for the stamp imprint per equipped stamp style. */
export const STAMP_STYLES: Record<string, string> = {
  default: 'border-primary text-primary dark:border-blush dark:text-blush',
  stamp_mint: 'border-mint text-mint',
  stamp_sky: 'border-[#2f6fd6] text-[#2f6fd6] dark:border-sky dark:text-sky',
  stamp_double:
    'border-primary text-primary outline-2 outline-offset-2 outline-current dark:border-blush dark:text-blush',
  stamp_gold:
    'border-[#b8892b] text-[#b8892b] dark:border-[#e4ae39] dark:text-[#e4ae39]',
  stamp_wax:
    'border-[#7d1830] bg-[#a3243f] text-white shadow-[inset_0_-3px_6px_rgb(0_0_0/30%)] mix-blend-normal dark:mix-blend-normal',
  stamp_rainbow: 'border-primary text-primary stamp-rainbow dark:border-blush dark:text-blush',
  stamp_checksum: 'border-dashed border-[#2b8a3e] text-[#2b8a3e] dark:border-[#38d27a] dark:text-[#38d27a]',
}

export function stampStyle(stampId: string | null | undefined) {
  return STAMP_STYLES[stampId ?? 'default'] ?? STAMP_STYLES.default
}

/** Three-colour swatches (surface, card, border) shown for UI skins. */
export const SKIN_SWATCHES: Record<string, [string, string, string]> = {
  skin_oat: ['#f1e7d3', '#fbf5e9', '#e2d3b8'],
  skin_matcha: ['#e9f0df', '#f8fcf2', '#7fa36a'],
  skin_kraft: ['#e3cfab', '#f3e6cd', '#a07c47'],
  skin_mocha: ['#e6d6ca', '#f6ede6', '#6b4433'],
  skin_terminal: ['#050906', '#0d1510', '#38d27a'],
  skin_noc: ['#070d1a', '#0f1a2e', '#4dabf7'],
}

export function initials(name: string) {
  const parts = name.trim().split(/\s+/).filter(Boolean)
  if (parts.length === 0) return '?'
  if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase()
  return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase()
}

/** Picks an item the way the server does: rarity by weight, then uniformly. */
export function pickWeighted(
  items: CosmeticItem[],
  rarities: RarityInfo[],
  random: () => number,
): CosmeticItem {
  const total = rarities.reduce((sum, rarity) => sum + rarity.drop_chance_percent, 0)
  let roll = random() * total
  let chosen: Rarity = rarities[0]?.rarity ?? 'common'
  for (const rarity of rarities) {
    if (roll < rarity.drop_chance_percent) {
      chosen = rarity.rarity
      break
    }
    roll -= rarity.drop_chance_percent
  }
  const pool = items.filter((item) => item.rarity === chosen)
  const candidates = pool.length > 0 ? pool : items
  return candidates[Math.floor(random() * candidates.length) % candidates.length]
}

export const REEL_LENGTH = 56
export const REEL_WINNER_INDEX = 48

/**
 * Builds the strip of tiles for the opening animation: filler items rolled
 * with the real drop rates, and the actual prize at `REEL_WINNER_INDEX`.
 */
export function buildReel(
  items: CosmeticItem[],
  rarities: RarityInfo[],
  winner: CosmeticItem | null,
  random: () => number = Math.random,
): CosmeticItem[] {
  if (items.length === 0) return []
  const reel: CosmeticItem[] = []
  for (let index = 0; index < REEL_LENGTH; index += 1) {
    if (index === REEL_WINNER_INDEX && winner) {
      reel.push(winner)
      continue
    }
    // Avoid the same item twice in a row; it looks broken on a spinning reel.
    let next = pickWeighted(items, rarities, random)
    for (let attempt = 0; attempt < 4 && next.id === reel[index - 1]?.id; attempt += 1) {
      next = pickWeighted(items, rarities, random)
    }
    reel.push(next)
  }
  return reel
}

/** Offset (px, negative) that centres tile `index` under the marker, plus jitter. */
export function reelOffset({
  index,
  tileWidth,
  gap,
  viewportWidth,
  jitter,
}: {
  index: number
  tileWidth: number
  gap: number
  viewportWidth: number
  jitter: number
}) {
  const pitch = tileWidth + gap
  return -(index * pitch + tileWidth / 2 - viewportWidth / 2 + jitter)
}
