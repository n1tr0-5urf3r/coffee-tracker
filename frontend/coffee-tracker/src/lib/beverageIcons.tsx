import { createElement } from 'react'
import { Coffee, CupSoda, Leaf, Popsicle, type LucideIcon } from 'lucide-react'

const iconsById: Record<string, LucideIcon> = {
  coffee: Coffee,
  espresso_pads: CupSoda,
  eis: Popsicle,
  tea: Leaf,
}

export function beverageIcon(beverageId: string): LucideIcon {
  return iconsById[beverageId] ?? CupSoda
}

/** Very short, stamp-sized label for a beverage. */
export function stampLabel(beverageId: string, beverageName: string) {
  if (beverageId === 'coffee') return 'Coffee'
  if (beverageId === 'espresso_pads') return 'Espresso'
  if (beverageId === 'eis') return 'Ice cream'
  if (beverageId === 'tea') return 'Tea'
  return beverageName
}

/** Renders the icon of a beverage; keeps the lookup out of the render tree. */
export function BeverageIcon({
  beverageId,
  className,
}: {
  beverageId: string
  className?: string
}) {
  return createElement(beverageIcon(beverageId), {
    className,
    'aria-hidden': 'true',
  })
}
