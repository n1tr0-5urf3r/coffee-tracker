import { Coffee } from 'lucide-react'
import { branding } from '../config/branding'
import { cn } from '../lib/cn'

export function BrandWatermark({ className }: { className?: string }) {
  if (branding.logoUrl) {
    return <img src={branding.logoUrl} alt="" className={className} />
  }

  return <Coffee className={cn('text-primary', className)} aria-hidden="true" />
}
