import confetti from 'canvas-confetti'
import { AnimatePresence, motion } from 'motion/react'
import {
  Bean,
  CalendarCheck,
  Check,
  FerrisWheel,
  Flame,
  Gift,
  Hammer,
  HandCoins,
  Milestone,
  TreePine,
  Lock,
  PackageOpen,
  ShoppingBasket,
  Sparkles,
  Stamp,
  Trophy,
  Volume2,
  VolumeX,
  WashingMachine,
} from 'lucide-react'
import { useQueryClient } from '@tanstack/react-query'
import { useEffect, useMemo, useRef, useState } from 'react'
import { Link } from 'react-router-dom'
import { ApiError, type CosmeticItem, type CosmeticSlot, type LootboxOpenResponse } from '../api/client'
import {
  queryKeys,
  useCraftMutation,
  useEquipMutation,
  useLocker,
  useLockerUpdate,
  useMe,
  useOpenLootboxMutation,
} from '../api/queries'
import { Button } from '../components/Button'
import { Card, Skeleton } from '../components/Card'
import { CosmeticPreview } from '../components/CosmeticPreview'
import { LootboxReel, RarityChip } from '../components/LootboxReel'
import { PageHeader } from '../components/PageHeader'
import { UserAvatar } from '../components/UserAvatar'
import { cn } from '../lib/cn'
import {
  RARITY_COLORS,
  RARITY_ORDER,
  SLOT_LABELS,
  SLOT_ORDER,
  equippedAvatarArtwork,
} from '../lib/cosmetics'
import { playCharge } from '../lib/lootSounds'
import { useToast } from '../lib/toast'

const SOUND_KEY = 'coffee-loot-sound'

function readSound() {
  try {
    return localStorage.getItem(SOUND_KEY) !== 'off'
  } catch {
    return true
  }
}

export function LockerPage() {
  const { data: me } = useMe()
  const { data: locker, isLoading } = useLocker()
  const openMutation = useOpenLootboxMutation()
  const equipMutation = useEquipMutation()
  const craftMutation = useCraftMutation()
  const applyLocker = useLockerUpdate()
  const queryClient = useQueryClient()
  const { pushToast } = useToast()

  const reducedMotion = useMemo(
    () => window.matchMedia('(prefers-reduced-motion: reduce)').matches,
    [],
  )
  const [sound, setSound] = useState(readSound)
  const [spinKey, setSpinKey] = useState(0)
  const [result, setResult] = useState<LootboxOpenResponse | null>(null)
  const [revealed, setRevealed] = useState(false)
  const [charging, setCharging] = useState(false)
  const [filter, setFilter] = useState<CosmeticSlot | 'all'>('all')

  const spinning = charging || openMutation.isPending || (result !== null && !revealed)

  // If the page is left mid-spin, still apply the result so counts stay right.
  const pendingResult = useRef<LootboxOpenResponse | null>(null)
  useEffect(
    () => () => {
      if (pendingResult.current) applyLocker(pendingResult.current, true)
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [],
  )
  const name = me?.user.display_name ?? 'Coffee friend'
  const isMember = me?.user.user_type === 'member'
  const equippedTitle = locker?.items.find((item) => item.id === locker.equipped.title)?.name

  const toggleSound = () => {
    const next = !sound
    setSound(next)
    try {
      localStorage.setItem(SOUND_KEY, next ? 'on' : 'off')
    } catch {
      // per-device convenience only
    }
  }

  const open = async () => {
    setRevealed(false)
    // The crate rattles while the server rolls the prize; the reel only
    // starts once both are done so the build-up always plays out.
    setCharging(true)
    if (sound && !reducedMotion) playCharge()
    const rattle = new Promise((resolve) => window.setTimeout(resolve, reducedMotion ? 0 : 700))
    try {
      const [data] = await Promise.all([openMutation.mutateAsync(), rattle])
      pendingResult.current = data
      setResult(data)
      setSpinKey((key) => key + 1)
    } catch (error) {
      // Go back to the last reveal instead of hanging on "Rolling…".
      setRevealed(true)
      if (error instanceof ApiError && error.status === 409) {
        // Another tab or device used the crate; refresh the stale count.
        void queryClient.invalidateQueries({ queryKey: queryKeys.locker })
      }
      pushToast({
        tone: 'error',
        title: 'The box is stuck',
        description:
          error instanceof ApiError && error.status === 409
            ? 'No unopened lootboxes left. Earn more below.'
            : 'Could not open the lootbox. Try again in a moment.',
      })
    } finally {
      setCharging(false)
    }
  }

  const onReelDone = () => {
    setRevealed(true)
    if (!result) return
    pendingResult.current = null
    applyLocker(result, result.unlocked_achievements.length > 0)
    if (!reducedMotion) celebrate(result.item.rarity)
    for (const achievement of result.unlocked_achievements) {
      pushToast({
        tone: 'success',
        title: `Achievement: ${achievement.name}`,
        description: `${achievement.description} +1 lootbox`,
      })
    }
  }

  const equip = (item: CosmeticItem, equipped: boolean) => {
    equipMutation.mutate(
      { slot: item.slot, item_id: equipped ? null : item.id },
      {
        onSuccess: () =>
          pushToast({
            tone: 'success',
            title: equipped ? 'Back to the classic look' : `${item.name} equipped`,
            description: equipped ? 'Slot reset to default.' : 'Looking sharp.',
          }),
      },
    )
  }

  const craft = (item: CosmeticItem) => {
    craftMutation.mutate(item.id, {
      onSuccess: (data) => {
        pushToast({
          tone: 'success',
          title: `${item.name} crafted`,
          description: `−${item.craft_cost} beans. It's yours now.`,
        })
        for (const achievement of data.unlocked_achievements) {
          pushToast({
            tone: 'success',
            title: `Achievement: ${achievement.name}`,
            description: `${achievement.description} +1 lootbox`,
          })
        }
      },
      onError: (error) =>
        pushToast({
          tone: 'error',
          title: 'Crafting failed',
          description: error instanceof ApiError ? error.message : 'Try again in a moment.',
        }),
    })
  }

  if (isLoading || !locker) {
    return (
      <div>
        <PageHeader eyebrow="Lootboxes & skins" title="Locker" />
        <div className="grid gap-4 lg:grid-cols-[1.5fr_1fr]">
          <Skeleton className="h-96" />
          <Skeleton className="h-96" />
        </div>
      </div>
    )
  }

  const ownedCount = locker.items.filter((item) => item.owned).length
  const visibleItems = locker.items
    .filter((item) => filter === 'all' || item.slot === filter)
    .sort(
      (a, b) =>
        // Unlocked items first, then by type, rarity and name.
        Number(b.owned) - Number(a.owned) ||
        SLOT_ORDER.indexOf(a.slot) - SLOT_ORDER.indexOf(b.slot) ||
        RARITY_ORDER.indexOf(a.rarity) - RARITY_ORDER.indexOf(b.rarity) ||
        a.name.localeCompare(b.name),
    )

  return (
    <div>
      <PageHeader eyebrow="Lootboxes & skins" title="Locker">
        <div className="flex flex-wrap gap-2">
          <span className="inline-flex items-center gap-2 rounded-full bg-white px-4 py-2 text-sm font-black text-berry-700 shadow-soft dark:bg-berry-900 dark:text-rose-100">
            <Gift className="size-4 text-primary dark:text-blush" aria-hidden="true" />
            {locker.boxes_available} {locker.boxes_available === 1 ? 'box' : 'boxes'}
          </span>
          <span className="inline-flex items-center gap-2 rounded-full bg-white px-4 py-2 text-sm font-black text-berry-700 shadow-soft dark:bg-berry-900 dark:text-rose-100">
            <Bean className="size-4 text-[#8b5a2b]" aria-hidden="true" />
            {locker.beans} beans
          </span>
          <span className="inline-flex items-center rounded-full bg-white px-4 py-2 text-sm font-black text-berry-700 shadow-soft dark:bg-berry-900 dark:text-rose-100">
            {ownedCount}/{locker.items.length} collected
          </span>
          {isMember ? (
            <Link
              to="/wheel"
              className="inline-flex items-center gap-2 rounded-full bg-primary px-4 py-2 text-sm font-black text-white shadow-button transition hover:bg-secondary dark:bg-blush dark:text-berry-950"
            >
              <FerrisWheel className="size-4" aria-hidden="true" />
              Lucky wheel
            </Link>
          ) : null}
        </div>
      </PageHeader>

      <div className="grid gap-4 lg:grid-cols-[1.6fr_1fr]">
        <Card className="min-w-0 overflow-hidden">
          <div className="mb-4 flex items-start justify-between gap-3">
            <div>
              <p className="font-display text-2xl font-black">Coffee crate</p>
              <p className="text-sm font-bold text-berry-700 dark:text-rose-100">
                Cosmetic only. Duplicates turn into beans.
              </p>
            </div>
            <button
              type="button"
              onClick={toggleSound}
              className="grid size-10 shrink-0 place-items-center rounded-full text-berry-700 hover:bg-primary/10 dark:text-rose-100 dark:hover:bg-white/10"
              aria-label={sound ? 'Mute reel sound' : 'Unmute reel sound'}
              aria-pressed={sound}
            >
              {sound ? <Volume2 className="size-5" /> : <VolumeX className="size-5" />}
            </button>
          </div>

          <LootboxReel
            charging={charging}
            items={locker.items}
            rarities={locker.rarities}
            winner={result?.item ?? null}
            spinKey={spinKey}
            reducedMotion={reducedMotion}
            sound={sound}
            onDone={onReelDone}
          />

          <div className="mt-4 min-h-28" aria-live="polite">
            <AnimatePresence mode="wait">
              {result && revealed ? (
                <RevealCard key={spinKey} result={result} />
              ) : spinning ? (
                <motion.p
                  key={charging ? 'charging' : 'spinning'}
                  initial={{ opacity: 0, scale: 0.9 }}
                  animate={{ opacity: 1, scale: 1 }}
                  exit={{ opacity: 0 }}
                  className="py-8 text-center font-display text-lg font-black uppercase tracking-[0.2em] text-berry-700 dark:text-rose-100"
                >
                  {charging ? 'Unlocking crate…' : 'Rolling…'}
                </motion.p>
              ) : (
                <motion.p
                  key="idle"
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                  className="py-8 text-center font-bold text-berry-700 dark:text-rose-100"
                >
                  {locker.boxes_available > 0
                    ? 'A crate is waiting. Feeling lucky?'
                    : 'No crates right now. Keep logging drinks, keep your streak, or do a chore at the machine to earn the next one.'}
                </motion.p>
              )}
            </AnimatePresence>
          </div>

          <div className="mt-2 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <Button
              onClick={() => void open()}
              disabled={spinning || locker.boxes_available === 0}
              className={cn(
                'sm:min-w-56',
                !spinning && locker.boxes_available > 0 && !reducedMotion && 'loot-pulse',
              )}
            >
              <PackageOpen className="size-5" aria-hidden="true" />
              {locker.boxes_available > 0
                ? `Open crate (${locker.boxes_available} left)`
                : 'No crates left'}
            </Button>
            {result && revealed && !result.duplicate && result.item.slot !== 'collectible' ? (
              <Button
                variant="secondary"
                onClick={() => equip(result.item, false)}
                disabled={
                  equipMutation.isPending ||
                  // Live state, not the (stale) open response.
                  Boolean(locker.items.find((item) => item.id === result.item.id)?.equipped)
                }
              >
                <Check className="size-4" aria-hidden="true" />
                Equip {result.item.name}
              </Button>
            ) : null}
          </div>
        </Card>

        <div className="space-y-4">
          <Card>
            <p className="mb-4 font-display text-xl font-black">Your loadout</p>
            <div className="flex items-center gap-4">
              <UserAvatar
                name={name}
                artworkPath={equippedAvatarArtwork(locker)}
                frameId={locker.equipped.frame}
                size="lg"
              />
              <div className="min-w-0">
                <p className="truncate font-display text-xl font-black">{name}</p>
                {equippedTitle ? (
                  <p className="font-display text-sm font-black italic text-primary dark:text-blush">
                    “{equippedTitle}”
                  </p>
                ) : (
                  <p className="text-sm font-bold text-berry-700 dark:text-rose-100">No title yet</p>
                )}
              </div>
            </div>
          </Card>

          <Card>
            <p className="mb-3 font-display text-xl font-black">How to earn crates</p>
            <ul className="space-y-2 text-sm font-bold text-berry-700 dark:text-rose-100">
              <SourceRow icon={<CalendarCheck className="size-4" />} label="Every 5th day you log a drink" count={locker.sources.daily} />
              <SourceRow icon={<Flame className="size-4" />} label="Streak bonus every 5 lab days" count={locker.sources.streak} />
              <SourceRow icon={<Milestone className="size-4" />} label="Every 25th drink" count={locker.sources.milestone} />
              <SourceRow icon={<Trophy className="size-4" />} label="Achievement unlocked" count={locker.sources.achievement} />
              <SourceRow icon={<Stamp className="size-4" />} label="Full Stempelkarte" count={locker.sources.stamp_card} />
              <SourceRow icon={<HandCoins className="size-4" />} label="Paying your tab" count={locker.sources.payment} />
              <SourceRow icon={<ShoppingBasket className="size-4" />} label="Shopping for the team" count={locker.sources.shopping} />
              <SourceRow icon={<WashingMachine className="size-4" />} label="Weekly machine chores" count={locker.sources.cleaning} />
              <SourceRow icon={<TreePine className="size-4" />} label="Advent calendar (1–24 Dec)" count={locker.sources.advent} />
              {isMember ? (
                <SourceRow icon={<FerrisWheel className="size-4" />} label="Lucky wheel prizes" count={locker.sources.wheel} />
              ) : null}
            </ul>
          </Card>

          <Card>
            <p className="mb-3 font-display text-xl font-black">Drop rates</p>
            <ul className="space-y-2">
              {locker.rarities.map((rarity) => (
                <li key={rarity.rarity} className="flex items-center justify-between gap-3 text-sm font-bold">
                  <RarityChip rarity={rarity.rarity} label={rarity.label} />
                  <span className="tabular-nums text-berry-700 dark:text-rose-100">
                    {rarity.drop_chance_percent.toFixed(rarity.drop_chance_percent < 10 ? 1 : 0)} % ·
                    dupe +{rarity.duplicate_beans} · craft {rarity.craft_cost}
                  </span>
                </li>
              ))}
            </ul>
          </Card>
        </div>
      </div>

      <section className="mt-8">
        <div className="mb-4 flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
          <h2 className="font-display text-2xl font-black">Collection</h2>
          <div className="flex flex-wrap gap-2" role="tablist" aria-label="Filter by type">
            {(['all', ...SLOT_ORDER] as const).map((slot) => (
              <button
                key={slot}
                type="button"
                role="tab"
                aria-selected={filter === slot}
                onClick={() => setFilter(slot)}
                className={cn(
                  'rounded-full px-3 py-1.5 text-xs font-black transition',
                  filter === slot
                    ? 'bg-primary text-white dark:bg-blush dark:text-berry-950'
                    : 'bg-white text-berry-700 shadow-soft hover:bg-primary/10 dark:bg-berry-900 dark:text-rose-100',
                )}
              >
                {slot === 'all' ? 'All' : SLOT_LABELS[slot]}
              </button>
            ))}
          </div>
        </div>
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
          {visibleItems.map((item) => (
            <ItemCard
              key={item.id}
              item={item}
              beans={locker.beans}
              busy={equipMutation.isPending || craftMutation.isPending}
              onEquip={() => equip(item, item.equipped)}
              onCraft={() => craft(item)}
            />
          ))}
        </div>
      </section>
    </div>
  )
}

function SourceRow({ icon, label, count }: { icon: React.ReactNode; label: string; count: number }) {
  return (
    <li className="flex items-center justify-between gap-3">
      <span className="flex items-center gap-2">
        <span className="text-primary dark:text-blush" aria-hidden="true">
          {icon}
        </span>
        {label}
      </span>
      <span className="rounded-full bg-rose-soft px-2 py-0.5 text-xs font-black tabular-nums text-primary dark:bg-white/10 dark:text-blush">
        {count}
      </span>
    </li>
  )
}

function RevealCard({ result }: { result: LootboxOpenResponse }) {
  const rarity = result.item.rarity
  const color = RARITY_COLORS[rarity]
  const big = rarity === 'epic' || rarity === 'legendary'
  return (
    <motion.div
      initial={{ opacity: 0, scale: 0.6, y: 12 }}
      animate={{ opacity: 1, scale: 1, y: 0 }}
      exit={{ opacity: 0 }}
      transition={{ type: 'spring', stiffness: 320, damping: big ? 12 : 18 }}
      className="relative flex items-center gap-4 overflow-hidden rounded-3xl border-2 p-4"
      style={{ borderColor: color, boxShadow: `0 0 ${big ? 48 : 28}px ${color}66` }}
    >
      <div className="relative grid w-24 shrink-0 place-items-center">
        {/* God rays behind the prize. */}
        <span
          aria-hidden="true"
          className="loot-rays pointer-events-none absolute size-40 rounded-full opacity-60"
          style={{
            background: `repeating-conic-gradient(${color}aa 0deg 10deg, transparent 10deg 30deg)`,
            maskImage: 'radial-gradient(circle, black 20%, transparent 70%)',
          }}
        />
        <motion.div
          initial={{ scale: 0.3, rotate: -12 }}
          animate={{ scale: 1, rotate: 0 }}
          transition={{ type: 'spring', stiffness: 400, damping: 10, delay: 0.05 }}
          className="relative"
        >
          <CosmeticPreview item={result.item} size="sm" className="w-24" />
        </motion.div>
      </div>
      <div className="relative min-w-0">
        <div className="mb-1 flex flex-wrap items-center gap-2">
          <RarityChip rarity={rarity} />
          {result.duplicate ? (
            <span className="inline-flex items-center gap-1 text-xs font-black text-berry-700 dark:text-rose-100">
              <Bean className="size-3 text-[#8b5a2b]" aria-hidden="true" /> Duplicate · +{result.beans_awarded} beans
            </span>
          ) : (
            <motion.span
              initial={{ scale: 0, rotate: -20 }}
              animate={{ scale: [0, 1.35, 1], rotate: [-20, 8, 0] }}
              transition={{ duration: 0.55, delay: 0.2 }}
              className="inline-flex items-center gap-1 rounded-full bg-mint px-2 py-0.5 text-xs font-black uppercase tracking-wide text-berry-950"
            >
              <Sparkles className="size-3" aria-hidden="true" /> New!
            </motion.span>
          )}
        </div>
        <p
          className={cn(
            'truncate font-display text-2xl font-black',
            rarity === 'legendary' &&
              'bg-gradient-to-r from-[#9a6a0c] via-[#e4ae39] to-[#9a6a0c] bg-clip-text text-transparent dark:from-[#e4ae39] dark:via-[#fff1b8] dark:to-[#e4ae39]',
          )}
        >
          {result.item.name}
        </p>
        <p className="text-sm font-bold text-berry-700 dark:text-rose-100">{result.item.description}</p>
      </div>
    </motion.div>
  )
}

/** Confetti that scales with rarity; legendaries get a multi-burst shower. */
function celebrate(rarity: CosmeticItem['rarity']) {
  const colors = [RARITY_COLORS[rarity], '#c2284a', '#ffd166', '#ffffff']
  const burst = (particleCount: number, x: number, spread = 70) =>
    void confetti({ particleCount, spread, startVelocity: 45, origin: { x, y: 0.4 }, colors })
  if (rarity === 'common') return burst(30, 0.5, 50)
  if (rarity === 'rare') return burst(70, 0.5)
  if (rarity === 'epic') {
    burst(90, 0.35)
    burst(90, 0.65)
    return
  }
  burst(140, 0.5, 100)
  window.setTimeout(() => burst(90, 0.25), 250)
  window.setTimeout(() => burst(90, 0.75), 450)
  window.setTimeout(() => burst(120, 0.5, 120), 800)
}

function ItemCard({
  item,
  beans,
  busy,
  onEquip,
  onCraft,
}: {
  item: CosmeticItem
  beans: number
  busy: boolean
  onEquip: () => void
  onCraft: () => void
}) {
  const color = RARITY_COLORS[item.rarity]
  return (
    <div
      className={cn(
        'flex flex-col overflow-hidden rounded-3xl border-2 bg-card shadow-soft dark:bg-berry-900',
        item.equipped ? 'border-primary dark:border-blush' : 'border-berry-100 dark:border-white/10',
      )}
      style={{ boxShadow: `inset 0 -5px 0 ${color}` }}
    >
      <div className={cn('relative p-3', !item.owned && 'opacity-45 grayscale')}>
        <CosmeticPreview item={item} size="md" />
        {!item.owned ? (
          <Lock className="absolute right-3 top-3 size-4 text-berry-700 dark:text-rose-100" aria-hidden="true" />
        ) : null}
      </div>
      <div className="flex grow flex-col gap-2 px-3 pb-4">
        <div className="flex items-center justify-between gap-2">
          <RarityChip rarity={item.rarity} />
          <span className="text-[0.65rem] font-black uppercase tracking-[0.12em] text-berry-700 dark:text-rose-100">
            {SLOT_LABELS[item.slot].replace(/s$/, '')}
          </span>
        </div>
        <p className="font-display text-sm font-black leading-tight">{item.name}</p>
        <p className="grow text-xs font-bold text-berry-700 dark:text-rose-100">{item.description}</p>
        {item.owned ? (
          item.slot === 'collectible' ? (
            <span className="text-xs font-black text-mint">In your collection</span>
          ) : (
            <Button
              variant={item.equipped ? 'primary' : 'secondary'}
              className="min-h-9 rounded-2xl px-3 py-2 text-xs"
              onClick={onEquip}
              disabled={busy}
            >
              {item.equipped ? (
                <>
                  <Check className="size-3" aria-hidden="true" /> Equipped
                </>
              ) : (
                'Equip'
              )}
            </Button>
          )
        ) : item.craftable && item.craft_cost !== null && item.craft_cost !== undefined ? (
          <Button
            variant="ghost"
            className="min-h-9 rounded-2xl px-3 py-2 text-xs ring-2 ring-berry-100 dark:ring-white/10"
            onClick={onCraft}
            disabled={busy || beans < item.craft_cost}
            title={beans < item.craft_cost ? `You need ${item.craft_cost - beans} more beans` : undefined}
          >
            <Hammer className="size-3" aria-hidden="true" /> Craft · {item.craft_cost}
          </Button>
        ) : (
          <span className="text-xs font-black text-berry-700 dark:text-rose-100">Crate drop only</span>
        )}
      </div>
    </div>
  )
}
