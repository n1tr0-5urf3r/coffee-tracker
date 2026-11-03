import { useState } from 'react'
import { cn } from '../lib/cn'
import { artworkUrl, initials } from '../lib/cosmetics'

const SIZES = {
  xs: 'size-7 text-[0.6rem]',
  sm: 'size-9 text-xs',
  md: 'size-12 text-sm',
  lg: 'size-20 text-xl',
  xl: 'size-28 text-3xl',
} as const

/** Profile picture (equipped avatar or initials) inside the equipped frame. */
export function UserAvatar({
  name,
  artworkPath,
  frameId,
  size = 'md',
  className,
}: {
  name: string
  /** Artwork path from the API (`artwork_path` / `avatar_artwork`). */
  artworkPath?: string | null
  frameId?: string | null
  size?: keyof typeof SIZES
  className?: string
}) {
  const [failedSrc, setFailedSrc] = useState<string | null>(null)
  const src = artworkPath ? artworkUrl(artworkPath) : null
  const showImage = src !== null && failedSrc !== src

  return (
    <span
      className={cn('cosmetic-frame inline-block shrink-0', !frameId && 'p-0', className)}
      data-frame={frameId ?? undefined}
    >
      <span
        className={cn(
          'grid place-items-center overflow-hidden rounded-full bg-rose-soft font-display font-black text-primary ring-2 ring-white dark:bg-berry-900 dark:text-blush dark:ring-berry-950',
          SIZES[size],
        )}
      >
        {showImage ? (
          <img
            src={src}
            alt=""
            loading="lazy"
            className="size-full object-cover"
            onError={() => setFailedSrc(src)}
          />
        ) : (
          <span aria-hidden="true">{initials(name)}</span>
        )}
      </span>
    </span>
  )
}
