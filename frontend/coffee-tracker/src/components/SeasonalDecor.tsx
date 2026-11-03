import type { CSSProperties } from 'react'
import { cn } from '../lib/cn'
import { useSeasonalTheme } from '../lib/seasonalTheme'
import { SEASONS } from '../lib/seasons'

type Particle = {
  left: number // vw %
  top: number // vh %, only used by the twinkle motion
  size: number // rem
  delay: number // s, negative so the screen is populated on load
  duration: number // s
  opacity: number
}

// Fixed table instead of Math.random() so particles stay put across re-renders.
// Entries after MOBILE_PARTICLES are hidden on small screens.
const PARTICLES: Particle[] = [
  { left: 4, top: 18, size: 1.4, delay: -2, duration: 19, opacity: 0.5 },
  { left: 14, top: 62, size: 1.1, delay: -11, duration: 23, opacity: 0.4 },
  { left: 27, top: 34, size: 1.6, delay: -6, duration: 21, opacity: 0.45 },
  { left: 41, top: 80, size: 1.2, delay: -15, duration: 26, opacity: 0.35 },
  { left: 56, top: 12, size: 1.5, delay: -4, duration: 20, opacity: 0.5 },
  { left: 69, top: 48, size: 1.1, delay: -18, duration: 24, opacity: 0.4 },
  { left: 83, top: 26, size: 1.7, delay: -9, duration: 22, opacity: 0.55 },
  { left: 94, top: 70, size: 1.3, delay: -13, duration: 25, opacity: 0.45 },
  { left: 9, top: 88, size: 1.2, delay: -20, duration: 27, opacity: 0.35 },
  { left: 21, top: 8, size: 1.5, delay: -1, duration: 18, opacity: 0.5 },
  { left: 35, top: 56, size: 1, delay: -16, duration: 28, opacity: 0.35 },
  { left: 63, top: 92, size: 1.4, delay: -7, duration: 21, opacity: 0.45 },
  { left: 76, top: 4, size: 1.2, delay: -22, duration: 26, opacity: 0.4 },
  { left: 89, top: 44, size: 1.5, delay: -3, duration: 23, opacity: 0.5 },
]
const MOBILE_PARTICLES = 8

/** Ambient, decorative particle layer. Sits behind page content and ignores pointer events. */
export function SeasonalDecor({ className }: { className?: string }) {
  const { activeSeason } = useSeasonalTheme()
  if (!activeSeason) return null

  const season = SEASONS[activeSeason]

  return (
    <div
      aria-hidden="true"
      className={cn('season-overlay pointer-events-none fixed inset-0 overflow-hidden', className)}
    >
      {PARTICLES.map((particle, index) => (
        <span
          key={`${activeSeason}-${index}`}
          data-motion={season.motion}
          className={cn('season-particle', index >= MOBILE_PARTICLES && 'hidden sm:block')}
          style={
            {
              left: `${particle.left}%`,
              top: season.motion === 'twinkle' ? `${particle.top}%` : undefined,
              fontSize: `${particle.size}rem`,
              animationDelay: `${particle.delay}s`,
              animationDuration: `${season.motion === 'twinkle' ? particle.duration / 5 : particle.duration}s`,
              '--particle-opacity': particle.opacity,
            } as CSSProperties
          }
        >
          {season.particles[index % season.particles.length]}
        </span>
      ))}
    </div>
  )
}

/** Small header pill naming the active season. */
export function SeasonBadge({ className }: { className?: string }) {
  const { activeSeason } = useSeasonalTheme()
  if (!activeSeason) return null

  const season = SEASONS[activeSeason]

  return (
    <span
      className={cn(
        'inline-flex items-center gap-1.5 rounded-full bg-season-soft px-3 py-2 text-sm font-black text-berry-950 ring-2 ring-season/35 dark:bg-season/20 dark:text-berry-50',
        className,
      )}
    >
      <span aria-hidden="true">{season.emoji}</span>
      {season.label}
    </span>
  )
}
