import { Coffee, Stamp as StampIcon } from 'lucide-react'
import type { CosmeticItem } from '../api/client'
import { cn } from '../lib/cn'
import { SKIN_SWATCHES, stampStyle } from '../lib/cosmetics'
import { UserAvatar } from './UserAvatar'

/** Visual stand-in for any cosmetic, used in the locker grid and the lootbox reel. */
export function CosmeticPreview({
  item,
  size = 'md',
  className,
}: {
  item: CosmeticItem
  size?: 'sm' | 'md' | 'lg'
  className?: string
}) {
  const box = {
    sm: 'h-16 text-3xl',
    md: 'h-24 text-5xl',
    lg: 'h-36 text-7xl',
  }[size]
  const avatarSize = size === 'sm' ? 'md' : size === 'md' ? 'lg' : 'xl'

  return (
    <div className={cn('grid w-full place-items-center', box, className)}>
      {item.slot === 'avatar' ? (
        <UserAvatar name={item.name} artworkPath={item.artwork_path} size={avatarSize} />
      ) : item.slot === 'frame' ? (
        <span className="cosmetic-frame inline-block" data-frame={item.id}>
          <span
            className={cn(
              'grid place-items-center rounded-full bg-rose-soft text-primary ring-2 ring-white dark:bg-berry-900 dark:text-blush dark:ring-berry-950',
              size === 'sm' ? 'size-12' : size === 'md' ? 'size-20' : 'size-28',
            )}
          >
            <Coffee className={size === 'sm' ? 'size-5' : 'size-8'} aria-hidden="true" />
          </span>
        </span>
      ) : item.slot === 'cup' ? (
        <span aria-hidden="true" className="leading-none drop-shadow-sm">
          {item.glyph ?? '☕'}
        </span>
      ) : item.slot === 'stamp' ? (
        <span
          className={cn(
            'grid place-items-center rounded-full border-[3px]',
            size === 'sm' ? 'size-12' : size === 'md' ? 'size-20' : 'size-28',
            stampStyle(item.id),
          )}
        >
          <StampIcon className={size === 'sm' ? 'size-5' : 'size-8'} aria-hidden="true" />
        </span>
      ) : item.slot === 'title' ? (
        <span
          className={cn(
            'max-w-full rounded-full bg-berry-950 px-3 py-1.5 text-center font-display font-black italic leading-tight text-sun dark:bg-white/10',
            size === 'sm' ? 'text-[0.65rem]' : size === 'md' ? 'text-sm' : 'text-lg',
          )}
        >
          “{item.name}”
        </span>
      ) : item.slot === 'skin' ? (
        <span
          aria-hidden="true"
          className={cn(
            'flex overflow-hidden rounded-2xl ring-2 ring-berry-100 dark:ring-white/10',
            size === 'sm' ? 'h-10 w-16' : size === 'md' ? 'h-16 w-24' : 'h-24 w-36',
          )}
        >
          {(SKIN_SWATCHES[item.id] ?? ['#f7f3ea', '#fffdf8', '#eadfce']).map((color) => (
            <span key={color} className="h-full flex-1" style={{ background: color }} />
          ))}
          <span className="h-full w-2 bg-primary" />
        </span>
      ) : item.id === 'collectible_kn_museum_ticket' ? (
        <MuseumTicket size={size} />
      ) : (
        <span aria-hidden="true" className="leading-none drop-shadow-sm">
          {item.glyph ?? '🎁'}
        </span>
      )}
    </div>
  )
}

/** The KN museum ticket collectible. Deliberately looks like a paper ticket. */
export function MuseumTicket({ size = 'md' }: { size?: 'sm' | 'md' | 'lg' }) {
  return (
    <span
      className={cn(
        'relative flex items-stretch overflow-hidden rounded-xl bg-[#fff4d6] font-display text-[#5b3a22] shadow-soft ring-1 ring-[#e4ae39]',
        size === 'sm' ? 'h-10 w-20 text-[0.4rem]' : size === 'md' ? 'h-16 w-32 text-[0.55rem]' : 'h-24 w-48 text-xs',
      )}
    >
      <span className="flex flex-1 flex-col justify-center px-2 text-left leading-tight">
        <span className="font-black uppercase tracking-[0.18em]">KN Museum</span>
        <span className="font-bold opacity-75">Admit one</span>
      </span>
      <span className="flex items-center border-l-2 border-dashed border-[#e4ae39] bg-[#e4ae39]/20 px-2 font-black">
        5 €
      </span>
    </span>
  )
}
