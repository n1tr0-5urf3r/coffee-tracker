export type SeasonId =
  | 'newyear'
  | 'christmas'
  | 'easter'
  | 'halloween'
  | 'coffeeday'
  | 'winter'
  | 'spring'
  | 'summer'
  | 'autumn'

export type SeasonMotion = 'fall' | 'drift' | 'twinkle'

export type SeasonDefinition = {
  label: string
  /** Shown on the logo tile and in the header badge. */
  emoji: string
  /** Emoji used for the ambient particle layer. */
  particles: string[]
  motion: SeasonMotion
}

/** Accent colours live in index.css (`[data-season]`), never here. */
export const SEASONS: Record<SeasonId, SeasonDefinition> = {
  newyear: { label: 'Happy New Year', emoji: '🥂', particles: ['✨', '🎆', '✨', '🎇'], motion: 'twinkle' },
  christmas: { label: 'Advent', emoji: '🎄', particles: ['❄️', '⭐', '❄️', '🎄'], motion: 'fall' },
  easter: { label: 'Easter', emoji: '🐣', particles: ['🥚', '🐣', '🌷', '🥚'], motion: 'fall' },
  halloween: { label: 'Halloween', emoji: '🎃', particles: ['🦇', '🎃', '👻', '🦇'], motion: 'drift' },
  coffeeday: { label: 'Coffee Day', emoji: '☕', particles: ['☕', '🫘', '🫘', '☕'], motion: 'fall' },
  winter: { label: 'Winter', emoji: '⛄', particles: ['❄️', '❄️', '❄️', '⛄'], motion: 'fall' },
  spring: { label: 'Spring', emoji: '🌸', particles: ['🌸', '🌱', '🌸', '🌼'], motion: 'drift' },
  summer: { label: 'Summer', emoji: '☀️', particles: ['☀️', '🧊', '🌻', '🧊'], motion: 'drift' },
  autumn: { label: 'Autumn', emoji: '🍂', particles: ['🍂', '🍁', '🍂', '🌰'], motion: 'drift' },
}

export const SEASON_IDS = Object.keys(SEASONS) as SeasonId[]

const DAY_MS = 24 * 60 * 60 * 1000

/** Easter Sunday (Gregorian) via the Anonymous Gregorian / Meeus-Jones-Butcher algorithm. */
export function easterSunday(year: number): Date {
  const a = year % 19
  const b = Math.floor(year / 100)
  const c = year % 100
  const d = Math.floor(b / 4)
  const e = b % 4
  const f = Math.floor((b + 8) / 25)
  const g = Math.floor((b - f + 1) / 3)
  const h = (19 * a + b - d - g + 15) % 30
  const i = Math.floor(c / 4)
  const k = c % 4
  const l = (32 + 2 * e + 2 * i - h - k) % 7
  const m = Math.floor((a + 11 * h + 22 * l) / 451)
  const month = Math.floor((h + l - 7 * m + 114) / 31)
  const day = ((h + l - 7 * m + 114) % 31) + 1
  return new Date(year, month - 1, day)
}

function daysFromEaster(date: Date): number {
  const day = new Date(date.getFullYear(), date.getMonth(), date.getDate())
  // Rounding absorbs DST shifts between the two local midnights.
  return Math.round((day.getTime() - easterSunday(date.getFullYear()).getTime()) / DAY_MS)
}

/** Resolves the season for a local calendar date. Special events win over base seasons. */
export function resolveSeason(date: Date): SeasonId {
  const month = date.getMonth() + 1
  const day = date.getDate()

  if ((month === 12 && day === 31) || (month === 1 && day <= 2)) return 'newyear'
  if (month === 12 && day <= 26) return 'christmas'

  const easterOffset = daysFromEaster(date)
  if (easterOffset >= -7 && easterOffset <= 1) return 'easter'

  if ((month === 10 && day >= 24) || (month === 11 && day === 1)) return 'halloween'
  if (month === 10 && day === 1) return 'coffeeday'

  if (month === 12 || month <= 2) return 'winter'
  if (month <= 5) return 'spring'
  if (month <= 8) return 'summer'
  return 'autumn'
}

function isSeasonId(value: string | null): value is SeasonId {
  return value !== null && (SEASON_IDS as string[]).includes(value)
}

/** Reads a `?season=<id>` preview override from a URL query string. */
export function parseSeasonOverride(search: string): SeasonId | null {
  const value = new URLSearchParams(search).get('season')
  return isSeasonId(value) ? value : null
}
