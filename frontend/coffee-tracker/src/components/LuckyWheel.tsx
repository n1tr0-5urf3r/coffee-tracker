import {
  animate,
  motion,
  useMotionValue,
  useTransform,
  useVelocity,
  type AnimationPlaybackControls,
} from 'framer-motion'
import { type PointerEvent, type Ref, useEffect, useImperativeHandle, useRef, useState } from 'react'
import type { WheelPrize } from '../api/client'
import { cn } from '../lib/cn'
import { playWheelTick, playWheelWindup } from '../lib/lootSounds'
import {
  BUTTON_LAUNCH_SPEED,
  MIN_FLICK_SPEED,
  PRIZE_META,
  type RotationSample,
  type WheelSlice,
  easeOutQuart,
  launchSpeed,
  planFinalSpin,
  pointerAngle,
  releaseVelocity,
  sliceIndexAt,
  unwrapDelta,
} from '../lib/wheel'

const SIZE = 400
const C = SIZE / 2
const R = 184
const BULBS = 24
/** A cruise this long never ends on its own; the server answer stops it. */
const CRUISE_SECONDS = 120

type Phase = 'idle' | 'launching' | 'final'

export type LuckyWheelHandle = {
  /** Throws the wheel like a strong flick (used by the spin button). */
  launch: () => void
}

function point(angle: number, radius: number) {
  const rad = (angle * Math.PI) / 180
  return [C + radius * Math.sin(rad), C - radius * Math.cos(rad)] as const
}

function slicePath(start: number, end: number, radius = R) {
  const [x1, y1] = point(start, radius)
  const [x2, y2] = point(end, radius)
  const large = end - start > 180 ? 1 : 0
  return `M ${C} ${C} L ${x1} ${y1} A ${radius} ${radius} 0 ${large} 1 ${x2} ${y2} Z`
}

/**
 * Prize wheel you spin by hand: grab it, drag it around and let go with a
 * clockwise flick. A strong enough flick launches a spin (`onLaunch`), the
 * wheel keeps turning at the throw speed while the server draws, and once
 * `spin` arrives it hands that speed over to a long slowdown onto
 * `spin.prize`. A weak push just coasts (`onWeakSpin`). Pegs on the rim flick
 * the pointer and tick as they pass. `launch()` on the ref does the same for
 * the button.
 */
export function LuckyWheel({
  ref,
  slices,
  spin,
  pending,
  interactive,
  reducedMotion,
  sound,
  onLaunch,
  onWeakSpin,
  onBlockedFlick,
  onDone,
}: {
  ref?: Ref<LuckyWheelHandle>
  slices: WheelSlice[]
  spin: { prize: WheelPrize; key: number } | null
  /** True while the server is drawing the prize for a launched spin. */
  pending: boolean
  /** Whether a flick may start a (free or paid) spin right now. */
  interactive: boolean
  reducedMotion: boolean
  sound: boolean
  onLaunch: () => void
  /** A push too weak (or counter-clockwise) to launch while a spin is ready. */
  onWeakSpin?: () => void
  /** A real flick while no spin is ready (e.g. a paid spin not bought yet). */
  onBlockedFlick?: () => void
  onDone: () => void
}) {
  const rotation = useMotionValue(0)
  const velocity = useVelocity(rotation)
  const blur = useTransform(velocity, (v) => `blur(${Math.min(Math.abs(v) / 1500, 2.2).toFixed(2)}px)`)
  const flap = useMotionValue(0)
  const pointer = useRef<SVGPathElement>(null)
  const wrapper = useRef<HTMLDivElement>(null)
  const lastSlice = useRef(-1)
  const [spinning, setSpinning] = useState(false)
  const [grabbing, setGrabbing] = useState(false)
  const [landed, setLanded] = useState<number | null>(null)

  const phase = useRef<Phase>('idle')
  const controls = useRef<AnimationPlaybackControls | null>(null)
  const throwSpeed = useRef(BUTTON_LAUNCH_SPEED)
  const drag = useRef<{ angle: number; moved: number; samples: RotationSample[] } | null>(null)

  const latest = useRef({
    onDone,
    onLaunch,
    onWeakSpin,
    onBlockedFlick,
    sound,
    slices,
    interactive,
    reducedMotion,
  })
  useEffect(() => {
    latest.current = {
      onDone,
      onLaunch,
      onWeakSpin,
      onBlockedFlick,
      sound,
      slices,
      interactive,
      reducedMotion,
    }
  })

  const stopMotion = () => {
    controls.current?.stop()
    controls.current = null
  }

  /** Constant-speed turning while the server draws. */
  const cruise = (speed: number) => {
    stopMotion()
    controls.current = animate(rotation, rotation.get() + speed * CRUISE_SECONDS, {
      duration: CRUISE_SECONDS,
      ease: 'linear',
    })
  }

  /** Free-wheeling slowdown, no prize involved. */
  const coast = (speed: number) => {
    stopMotion()
    const duration = Math.min(Math.max(Math.abs(speed) / 450, 0.5), 2.2)
    // Quadratic ease-out starts at slope 2: distance = speed * duration / 2.
    controls.current = animate(rotation, rotation.get() + (speed * duration) / 2, {
      duration,
      ease: (t) => 1 - (1 - t) ** 2,
    })
  }

  const launch = (speed?: number) => {
    const { interactive: allowed, reducedMotion: still, sound: audible } = latest.current
    if (!allowed || phase.current !== 'idle') return
    phase.current = 'launching'
    throwSpeed.current = launchSpeed(speed ?? BUTTON_LAUNCH_SPEED)
    setLanded(null)
    latest.current.onLaunch()
    if (still) return
    setSpinning(true)
    if (speed !== undefined) {
      cruise(throwSpeed.current)
      return
    }
    // The button pulls the wheel back first, then lets it fly.
    if (audible) playWheelWindup()
    stopMotion()
    const windup = animate(rotation, rotation.get() - 24, { duration: 0.45, ease: [0.3, 0, 0.2, 1] })
    controls.current = windup
    void windup.then(() => {
      if (phase.current === 'launching' && controls.current === windup) cruise(throwSpeed.current)
    })
  }

  useImperativeHandle(ref, () => ({ launch: () => launch() }))

  // Each peg passing the pointer: tick, flick the pointer, tint it like the slice.
  useEffect(
    () =>
      rotation.on('change', (value) => {
        const index = sliceIndexAt(value, latest.current.slices)
        if (index === lastSlice.current) return
        lastSlice.current = index
        const slice = latest.current.slices[index]
        if (slice && pointer.current) pointer.current.style.fill = PRIZE_META[slice.prize].fill
        // Reduced motion jumps straight to the result: no peg chatter.
        if (latest.current.reducedMotion) return
        if (latest.current.sound) playWheelTick()
        flap.set(velocity.get() < 0 ? 24 : -24)
        animate(flap, 0, { type: 'spring', stiffness: 700, damping: 14 })
      }),
    [rotation, velocity, flap],
  )

  const spinKey = spin?.key
  const prize = spin?.prize
  useEffect(() => {
    if (spinKey === undefined || !prize) return
    const { slices: currentSlices, reducedMotion: still } = latest.current
    stopMotion()
    phase.current = 'final'
    const { target, duration } = planFinalSpin(rotation.get(), throwSpeed.current, currentSlices, prize)
    const finish = () => {
      phase.current = 'idle'
      setSpinning(false)
      setLanded(sliceIndexAt(target, currentSlices))
      latest.current.onDone()
    }
    if (still) {
      rotation.jump(target)
      const timer = window.setTimeout(finish, 0)
      return () => window.clearTimeout(timer)
    }
    setSpinning(true)
    const run = animate(rotation, target, { duration, ease: easeOutQuart })
    controls.current = run
    let cancelled = false
    void run.then(() => {
      if (!cancelled) finish()
    })
    return () => {
      cancelled = true
      run.stop()
    }
  }, [spinKey, prize, rotation])

  // The draw failed: let the wheel run out instead of spinning forever.
  useEffect(() => {
    if (pending || phase.current !== 'launching') return
    phase.current = 'idle'
    setSpinning(false)
    coast(throwSpeed.current)
    // Only the falling edge of `pending` matters; `coast` reads refs.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pending])

  useEffect(() => stopMotion, [])

  const angleOf = (event: PointerEvent) => {
    const box = wrapper.current?.getBoundingClientRect()
    if (!box) return null
    return pointerAngle(
      event.clientX - (box.left + box.width / 2),
      event.clientY - (box.top + box.height / 2),
      box.width * 0.1,
    )
  }

  const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (phase.current !== 'idle' || event.button !== 0) return
    const angle = angleOf(event)
    if (angle === null) return
    stopMotion()
    event.currentTarget.setPointerCapture(event.pointerId)
    drag.current = { angle, moved: 0, samples: [{ t: event.timeStamp, r: rotation.get() }] }
    setGrabbing(true)
    setLanded(null)
  }

  const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
    const current = drag.current
    if (!current) return
    const angle = angleOf(event)
    if (angle === null) return
    const step = unwrapDelta(current.angle, angle)
    current.angle = angle
    current.moved += Math.abs(step)
    const value = rotation.get() + step
    rotation.set(value)
    current.samples.push({ t: event.timeStamp, r: value })
    if (current.samples.length > 40) current.samples.shift()
  }

  const onPointerUp = (event: PointerEvent<HTMLDivElement>) => {
    const current = drag.current
    if (!current) return
    drag.current = null
    setGrabbing(false)
    const speed = releaseVelocity(current.samples, event.timeStamp)
    const flicked = speed >= MIN_FLICK_SPEED
    if (flicked && latest.current.interactive) {
      launch(speed)
      return
    }
    if (Math.abs(speed) > 30) coast(Math.max(Math.min(speed, 900), -900))
    if (flicked) latest.current.onBlockedFlick?.()
    else if (latest.current.interactive && current.moved > 15) latest.current.onWeakSpin?.()
  }

  // The browser took the gesture over (e.g. scrolling): never launch from it.
  const onPointerCancel = () => {
    if (!drag.current) return
    drag.current = null
    setGrabbing(false)
  }

  return (
    <div
      ref={wrapper}
      className={cn(
        'relative mx-auto aspect-square w-full max-w-[26rem] select-none',
        // Only capture touch while a spin is ready; otherwise swipes scroll the page.
        interactive ? 'touch-none' : 'touch-pan-y',
        grabbing ? 'cursor-grabbing' : 'cursor-grab',
      )}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerCancel}
    >
      {/* Rim with marquee bulbs (static, around the spinning disc). */}
      <svg viewBox={`0 0 ${SIZE} ${SIZE}`} className="absolute inset-0 size-full drop-shadow-[0_14px_24px_rgb(74_16_34/28%)]" aria-hidden="true">
        <circle cx={C} cy={C} r={199} fill="#4a1022" />
        <circle cx={C} cy={C} r={192} fill="none" stroke="#ffd166" strokeWidth={2} opacity={0.6} />
        {Array.from({ length: BULBS }, (_, index) => {
          const [x, y] = point((index * 360) / BULBS, 192)
          return (
            <circle
              key={index}
              cx={x}
              cy={y}
              r={4.2}
              className={cn('wheel-bulb', spinning && 'wheel-bulb-fast', index % 2 === 1 && 'wheel-bulb-alt')}
            />
          )
        })}
      </svg>

      <motion.div className="absolute inset-[4%]" style={{ rotate: rotation, filter: blur }}>
        <svg
          viewBox={`0 0 ${SIZE} ${SIZE}`}
          className="pointer-events-none size-full"
          role="img"
          aria-label="Lucky wheel. Grab it and flick it clockwise to spin."
        >
          {slices.map((slice, index) => {
            const meta = PRIZE_META[slice.prize]
            const width = slice.end - slice.start
            const mid = slice.start + width / 2
            const narrow = width < 20
            // Labels run along the radius; flip them on the right half so they never read upside down.
            const labelTurn = mid > 0 && mid < 180 ? -90 : 90
            return (
              <g key={`${slice.prize}-${index}`}>
                <path
                  d={slicePath(slice.start, slice.end, 200)}
                  fill={meta.fill}
                  stroke="#4a1022"
                  strokeWidth={2.5}
                  strokeLinejoin="round"
                />
                {landed === index ? (
                  <path
                    d={slicePath(slice.start, slice.end, 200)}
                    fill="#ffffff"
                    className="wheel-win-flash"
                  />
                ) : null}
                <g transform={`rotate(${mid} ${C} ${C})`}>
                  <text
                    x={C}
                    y={C - 168}
                    textAnchor="middle"
                    dominantBaseline="central"
                    fontSize={narrow ? 17 : 24}
                  >
                    {meta.glyph}
                  </text>
                  <text
                    x={C}
                    y={C - 108}
                    transform={`rotate(${labelTurn} ${C} ${C - 108})`}
                    textAnchor="middle"
                    dominantBaseline="central"
                    fontSize={narrow ? 12 : 15}
                    fontWeight={900}
                    fill={meta.ink}
                    className="font-display uppercase tracking-wide"
                  >
                    {meta.short}
                  </text>
                </g>
              </g>
            )
          })}
          {/* Pegs on every slice boundary. */}
          {slices.map((slice) => {
            const [x, y] = point(slice.start, 190)
            return <circle key={slice.start} cx={x} cy={y} r={5} fill="#fff7e0" stroke="#4a1022" strokeWidth={2} />
          })}
        </svg>
      </motion.div>

      {/* Hub. */}
      <div className="absolute left-1/2 top-1/2 grid size-[18%] -translate-x-1/2 -translate-y-1/2 place-items-center rounded-full border-4 border-[#ffd166] bg-gradient-to-br from-primary to-[#7a1430] text-2xl shadow-[0_4px_0_rgb(74_16_34/40%),inset_0_2px_6px_rgb(255_255_255/35%)] sm:text-3xl">
        <span aria-hidden="true">☕</span>
      </div>

      {/* Pointer / flapper at 12 o'clock. */}
      <motion.svg
        viewBox="0 0 40 56"
        className="absolute left-1/2 top-[-8.5%] z-10 w-[11%] -translate-x-1/2 drop-shadow-[0_3px_3px_rgb(0_0_0/35%)]"
        style={{ rotate: flap, originX: 0.5, originY: 0.2 }}
        aria-hidden="true"
      >
        <path
          ref={pointer}
          d="M20 54 L5 18 A16 16 0 1 1 35 18 Z"
          fill="#c2284a"
          stroke="#4a1022"
          strokeWidth={3}
          strokeLinejoin="round"
          style={{ transition: 'fill 120ms' }}
        />
        <circle cx={20} cy={14} r={5} fill="#ffd166" stroke="#4a1022" strokeWidth={2} />
      </motion.svg>
    </div>
  )
}
