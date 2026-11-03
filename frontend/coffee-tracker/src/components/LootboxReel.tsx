import { animate, motion, useMotionValue, useTransform, useVelocity } from 'framer-motion'
import { useEffect, useMemo, useRef, useState } from 'react'
import type { CosmeticItem, RarityInfo } from '../api/client'
import { cn } from '../lib/cn'
import {
  RARITY_COLORS,
  REEL_WINNER_INDEX,
  buildReel,
  reelOffset,
} from '../lib/cosmetics'
import { playLanding, playTick } from '../lib/lootSounds'
import { playImpactThud, retrigger, shakeScreen } from '../lib/reelFx'
import { CosmeticPreview } from './CosmeticPreview'
import './LootboxReel.css'

const TILE_WIDTH = 128
const GAP = 8
const PITCH = TILE_WIDTH + GAP
const SPIN_SECONDS = 6.8
/** Fast start, long nail-biting crawl at the end. */
const SPIN_EASE = [0.06, 0.72, 0.08, 1] as const
const PEAK_SPEED = 4200 // px/s, roughly the speed right after launch
/** The reel trembles for this long (s) right before it stops. */
const TREMBLE_SECONDS = 1.1
/** Below this speed (px/s) the marker flicks on every passing tile. */
const KICK_SPEED = 1100

/**
 * Horizontal case-opening reel that spins past a centre marker and stops on
 * `winner`. `spinKey` changes for every opening so the reel resets.
 * `charging` shakes the closed crate while the server rolls the prize.
 */
export function LootboxReel({
  items,
  rarities,
  winner,
  spinKey,
  charging,
  reducedMotion,
  sound,
  onDone,
}: {
  items: CosmeticItem[]
  rarities: RarityInfo[]
  winner: CosmeticItem | null
  spinKey: number
  charging: boolean
  reducedMotion: boolean
  sound: boolean
  onDone: () => void
}) {
  const viewport = useRef<HTMLDivElement>(null)
  const marker = useRef<HTMLDivElement>(null)
  const shaker = useRef<HTMLDivElement>(null)
  const lastTile = useRef(-1)
  const [landedKey, setLandedKey] = useState(-1)
  const landed = winner !== null && landedKey === spinKey

  const x = useMotionValue(0)
  const velocity = useVelocity(x)
  // Motion blur while the reel flies by.
  const filter = useTransform(velocity, (v) => `blur(${Math.min(Math.abs(v) / 1100, 3).toFixed(2)}px)`)

  const onDoneRef = useRef(onDone)
  const soundRef = useRef(sound)
  useEffect(() => {
    onDoneRef.current = onDone
    soundRef.current = sound
  })

  // A fresh strip per opening; the idle strip (no winner yet) is just filler.
  const reel = useMemo(
    () => buildReel(items, rarities, winner),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [spinKey, items.length, winner?.id],
  )

  // Tick and re-colour the marker whenever a new tile passes under it.
  useEffect(
    () =>
      x.on('change', (latest) => {
        if (!viewport.current) return
        const tile = Math.floor((-latest + viewport.current.clientWidth / 2) / PITCH)
        if (tile === lastTile.current) return
        lastTile.current = tile
        const item = reel[tile]
        if (item && marker.current) {
          const color = RARITY_COLORS[item.rarity]
          marker.current.style.background = color
          marker.current.style.boxShadow = `0 0 14px 2px ${color}`
          if (!reducedMotion && Math.abs(velocity.get()) < KICK_SPEED) {
            retrigger(marker.current, 'reel-marker-kick')
          }
        }
        if (soundRef.current) {
          const slowness = 1 - Math.min(Math.abs(velocity.get()) / PEAK_SPEED, 1)
          playTick(slowness)
        }
      }),
    [x, velocity, reel, reducedMotion],
  )

  useEffect(() => {
    if (!winner || !viewport.current) return
    const trembler = shaker.current
    const width = viewport.current.clientWidth
    // Land somewhere inside the tile, not always dead centre.
    const jitter = (Math.random() - 0.5) * (TILE_WIDTH - 28)
    const stopAt = reelOffset({
      index: REEL_WINNER_INDEX,
      tileWidth: TILE_WIDTH,
      gap: GAP,
      viewportWidth: width,
      jitter,
    })
    const finish = () => {
      trembler?.classList.remove('reel-tremble')
      setLandedKey(spinKey)
      if (soundRef.current) {
        playLanding(winner.rarity)
        playImpactThud(winner.rarity)
      }
      if (winner.rarity === 'legendary' && !reducedMotion) shakeScreen()
      if (winner.rarity === 'epic' || winner.rarity === 'legendary') {
        navigator.vibrate?.(winner.rarity === 'legendary' ? [40, 60, 40, 60, 120] : [40, 50, 80])
      }
      onDoneRef.current()
    }
    lastTile.current = -1
    x.jump(0)
    if (reducedMotion) {
      x.jump(stopAt)
      const timer = window.setTimeout(finish, 0)
      return () => window.clearTimeout(timer)
    }
    const controls = animate(x, stopAt, { duration: SPIN_SECONDS, ease: SPIN_EASE })
    // Build tension: a nervous tremble just before the reel stops.
    const trembleTimer = window.setTimeout(
      () => trembler?.classList.add('reel-tremble'),
      (SPIN_SECONDS - TREMBLE_SECONDS) * 1000,
    )
    let cancelled = false
    void controls.then(() => {
      if (!cancelled) finish()
    })
    return () => {
      cancelled = true
      window.clearTimeout(trembleTimer)
      trembler?.classList.remove('reel-tremble')
      controls.stop()
    }
  }, [spinKey, winner, reducedMotion, x])

  const winColor = winner ? RARITY_COLORS[winner.rarity] : '#ffd166'
  const bigWin = landed && (winner?.rarity === 'epic' || winner?.rarity === 'legendary')

  return (
    <div
      ref={viewport}
      className={cn(
        'relative h-48 overflow-hidden rounded-3xl bg-berry-950 ring-2 ring-berry-900 dark:ring-white/10',
        charging && !reducedMotion && 'loot-charge',
        bigWin && !reducedMotion && 'loot-impact',
      )}
      style={landed ? { boxShadow: `0 0 0 2px ${winColor}, 0 0 40px ${winColor}88` } : undefined}
      aria-hidden="true"
    >
      {/* Ambient glow that takes on the prize colour after landing. */}
      <div
        className="pointer-events-none absolute inset-0 transition-[background] duration-700"
        style={{
          background: landed
            ? `radial-gradient(circle at 50% 50%, ${winColor}55, transparent 60%)`
            : charging
              ? 'radial-gradient(circle at 50% 50%, rgb(255 209 102 / 25%), transparent 60%)'
              : 'transparent',
        }}
      />
      {/* Wrapper for the end-of-spin tremble; the strip itself is moved by `x`. */}
      <div ref={shaker} className="absolute inset-0">
        <motion.div
          className="absolute inset-y-0 left-0 flex items-center will-change-transform"
          style={{ gap: GAP, x, filter: reducedMotion ? undefined : filter }}
        >
          {reel.map((item, index) => {
            const isWinner = landed && index === REEL_WINNER_INDEX
            const color = RARITY_COLORS[item.rarity]
            return (
              <motion.div
                key={`${spinKey}-${index}`}
                animate={
                  landed
                    ? { scale: isWinner ? 1.1 : 0.94, opacity: isWinner ? 1 : 0.3 }
                    : { scale: 1, opacity: 1 }
                }
                transition={{ type: 'spring', stiffness: 300, damping: 16 }}
                className={cn(
                  'relative flex h-36 shrink-0 flex-col justify-between overflow-hidden rounded-2xl p-2',
                  isWinner && 'z-10',
                )}
                style={{
                  width: TILE_WIDTH,
                  boxShadow: isWinner
                    ? `inset 0 -6px 0 ${color}, 0 0 0 2px ${color}, 0 0 28px ${color}`
                    : `inset 0 -6px 0 ${color}`,
                  background: `linear-gradient(180deg, rgb(255 255 255 / 6%), ${color}${isWinner ? '66' : '33'})`,
                }}
              >
                {isWinner && !reducedMotion ? <span className="loot-shine" /> : null}
                <CosmeticPreview item={item} size="sm" className="h-20" />
                <p className="truncate pb-2 text-center text-[0.68rem] font-black text-white/90">
                  {item.name}
                </p>
              </motion.div>
            )
          })}
        </motion.div>
      </div>

      {/* Flash on impact. */}
      {landed && !reducedMotion ? (
        <motion.div
          key={`flash-${spinKey}`}
          className="pointer-events-none absolute inset-0"
          style={{ background: winColor }}
          initial={{ opacity: bigWin ? 0.75 : 0.4 }}
          animate={{ opacity: 0 }}
          transition={{ duration: bigWin ? 0.9 : 0.5, ease: 'easeOut' }}
        />
      ) : null}

      {landed && !reducedMotion && winner ? (
        <ImpactBurst key={`burst-${spinKey}`} color={winColor} big={bigWin} />
      ) : null}
      {bigWin && !reducedMotion && winner ? (
        <RarityBanner key={`banner-${spinKey}`} rarity={winner.rarity} color={winColor} />
      ) : null}

      {/* Centre marker (re-coloured live) and edge fades. */}
      <div
        ref={marker}
        className={cn(
          'pointer-events-none absolute inset-y-0 left-1/2 w-1 -translate-x-1/2 rounded-full bg-sun shadow-[0_0_12px_#ffd166] transition-opacity',
          landed && 'opacity-0',
        )}
      />
      <div className="pointer-events-none absolute left-1/2 top-0 size-0 -translate-x-1/2 border-x-[10px] border-t-[10px] border-x-transparent border-t-sun drop-shadow-[0_0_6px_#ffd166]" />
      <div className="pointer-events-none absolute bottom-0 left-1/2 size-0 -translate-x-1/2 border-x-[10px] border-b-[10px] border-x-transparent border-b-sun drop-shadow-[0_0_6px_#ffd166]" />
      <div className="pointer-events-none absolute inset-y-0 left-0 w-20 bg-gradient-to-r from-berry-950 to-transparent" />
      <div className="pointer-events-none absolute inset-y-0 right-0 w-20 bg-gradient-to-l from-berry-950 to-transparent" />
    </div>
  )
}

export function RarityChip({ rarity, label }: { rarity: CosmeticItem['rarity']; label?: string }) {
  return (
    <span
      className="inline-flex items-center rounded-full px-2 py-0.5 text-[0.65rem] font-black uppercase tracking-[0.12em] text-white"
      style={{ background: RARITY_COLORS[rarity] }}
    >
      {label ?? rarity}
    </span>
  )
}

const SPARKS = 14

/** Shockwave ring plus a burst of sparks from the winning tile. */
function ImpactBurst({ color, big }: { color: string; big: boolean }) {
  return (
    <div className="pointer-events-none absolute left-1/2 top-1/2 z-20" aria-hidden="true">
      {(big ? [0, 0.12] : [0]).map((delay) => (
        <motion.span
          key={delay}
          className="absolute -left-12 -top-12 size-24 rounded-full border-4"
          style={{ borderColor: color }}
          initial={{ scale: 0.4, opacity: 0.9 }}
          animate={{ scale: big ? 3.4 : 2.6, opacity: 0 }}
          transition={{ duration: 0.75, delay, ease: 'easeOut' }}
        />
      ))}
      {Array.from({ length: SPARKS }, (_, index) => {
        // Deterministic spread so renders stay pure.
        const angle = (index / SPARKS) * Math.PI * 2 + (index % 3) * 0.17
        const distance = (big ? 130 : 95) + (index % 4) * 14
        return (
          <motion.span
            key={index}
            className="absolute -left-1 -top-1 size-2 rounded-full"
            style={{ background: index % 3 === 0 ? '#ffffff' : color, boxShadow: `0 0 8px ${color}` }}
            initial={{ x: 0, y: 0, scale: 1, opacity: 1 }}
            animate={{
              x: Math.cos(angle) * distance,
              y: Math.sin(angle) * distance * 0.6,
              scale: 0.3,
              opacity: 0,
            }}
            transition={{ duration: 0.7 + (index % 3) * 0.12, ease: [0.15, 0.7, 0.3, 1] }}
          />
        )
      })}
    </div>
  )
}

/** "EPIC!" / "LEGENDARY!" slammed onto the reel for big wins. */
function RarityBanner({ rarity, color }: { rarity: CosmeticItem['rarity']; color: string }) {
  return (
    <motion.p
      aria-hidden="true"
      className="pointer-events-none absolute inset-x-0 top-2 z-30 text-center font-display text-3xl font-black uppercase tracking-[0.12em] text-white"
      style={{ textShadow: `0 0 18px ${color}, 0 3px 0 rgb(0 0 0 / 45%)` }}
      initial={{ scale: 2.4, opacity: 0, rotate: -6 }}
      animate={{ scale: [2.4, 0.95, 1, 1], opacity: [0, 1, 1, 0], rotate: [-6, 2, 0, 0] }}
      transition={{ duration: 2, times: [0, 0.14, 0.22, 1], ease: 'easeOut' }}
    >
      {rarity === 'legendary' ? 'Legendary!' : 'Epic!'}
    </motion.p>
  )
}
