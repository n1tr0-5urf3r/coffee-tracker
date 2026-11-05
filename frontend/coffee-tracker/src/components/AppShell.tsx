import { Navigate, NavLink, Outlet, useLocation, useNavigate } from 'react-router-dom'
import {
  BarChart3,
  Coffee,
  Banknote,
  FerrisWheel,
  Gift,
  LogOut,
  Menu,
  X,
  Medal,
  Settings,
  Stamp,
  Trophy,
} from 'lucide-react'
import { useEffect, useRef } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { api } from '../api/client'
import { useLocker, useMe, useWheel } from '../api/queries'
import { Button } from './Button'
import { LogoMark } from './LogoMark'
import { cn } from '../lib/cn'
import { AppFooter } from './AppFooter'
import { branding } from '../config/branding'
import { SeasonBadge, SeasonalDecor } from './SeasonalDecor'
import { UserAvatar } from './UserAvatar'
import { CrateReminder } from './CrateReminder'
import { useViewMode } from '../lib/viewMode'
import { equippedAvatarArtwork } from '../lib/cosmetics'

export function AppShell() {
  const { simple } = useViewMode()
  const menu = useRef<HTMLDialogElement>(null)
  const { data } = useMe()
  const { data: locker } = useLocker(Boolean(data))
  const boxes = locker?.boxes_available ?? 0
  const isMember = data?.user.user_type === 'member'
  const { data: wheel } = useWheel(isMember)
  const freeSpins = wheel?.free_spins_available ?? 0
  const equippedTitle = locker?.items.find((item) => item.id === locker.equipped.title)?.name
  const avatarArtwork = equippedAvatarArtwork(locker)
  const navigate = useNavigate()
  const location = useLocation()
  const queryClient = useQueryClient()
  const navItems = [
    { to: '/', label: 'Log', icon: Coffee },
    { to: '/stempelkarte', label: 'Stempelkarte', mobileLabel: 'Karte', icon: Stamp },
    { to: '/dashboard', label: 'Stats', icon: BarChart3 },
    { to: '/achievements', label: 'Badges', icon: Medal },
    { to: '/locker', label: 'Locker', icon: Gift, desktopOnly: true, badge: boxes },
    ...(isMember
      ? [{ to: '/wheel', label: 'Lucky wheel', icon: FerrisWheel, desktopOnly: true, badge: freeSpins }]
      : []),
    { to: '/leaderboard', label: `${branding.organizationShortName} Ranking`, mobileLabel: 'Rank', icon: Trophy },
    { to: '/payments', label: 'Payments', mobileLabel: 'Pay', icon: Banknote },
    { to: '/settings', label: 'Settings', icon: Settings },
  ]

  const simpleNavItems = navItems.filter((item) => ['/', '/dashboard', '/payments', '/settings'].includes(item.to))
  const hiddenInSimpleView = ['/stempelkarte', '/achievements', '/locker', '/wheel', '/leaderboard', '/components'].some(
    (path) => location.pathname === path || location.pathname.startsWith(`${path}/`),
  )

  useEffect(() => {
    const listener = () => {
      if (location.pathname !== '/login') {
        navigate('/login', { replace: true })
      }
    }
    window.addEventListener('coffee:unauthorized', listener)
    return () => window.removeEventListener('coffee:unauthorized', listener)
  }, [location.pathname, navigate])

  // The equipped UI skin re-tints surfaces app-wide (see index.css).
  const skin = locker?.equipped.skin ?? null
  useEffect(() => {
    const root = document.documentElement
    if (skin) {
      root.dataset.skin = skin
    } else {
      delete root.dataset.skin
    }
    return () => {
      delete root.dataset.skin
    }
  }, [skin])

  const logout = async () => {
    await api.logout()
    queryClient.clear()
    navigate('/login', { replace: true })
  }

  return (
    <div className={cn('relative isolate min-h-svh', !simple && 'pb-24 md:pb-0')}>
      {!simple && <SeasonalDecor className="z-0" />}
      <header className="sticky top-0 z-30 border-b border-white/50 bg-surface/82 backdrop-blur-xl dark:border-white/10 dark:bg-berry-950/82">
        <div className="mx-auto flex max-w-7xl items-center justify-between gap-4 px-4 py-3 sm:px-6">
          <LogoMark seasonal={!simple} className="min-w-0 flex-1" />
          <div className="ml-auto flex shrink-0 items-center gap-2">
            {simple && (
              <Button id="navigation-trigger" variant="secondary" onClick={() => menu.current?.showModal()} aria-haspopup="dialog" aria-controls="navigation-menu">
                <Menu className="size-4" aria-hidden="true" /> Menu
              </Button>
            )}
          </div>
          {!simple && <>
            {/* Phones have no room for a Locker tab, so the header carries a labelled entry. */}
            <NavLink
              to="/locker"
              aria-label={boxes > 0 ? `Locker, ${boxes} unopened crates` : 'Locker'}
              className={({ isActive }) =>
                cn(
                  'flex items-center gap-2 rounded-full py-1 pl-3 pr-1 font-display text-xs font-black shadow-soft transition md:hidden',
                  isActive
                    ? 'bg-primary text-white dark:bg-blush dark:text-berry-950'
                    : boxes > 0
                      ? 'bg-sun text-berry-950'
                      : 'bg-white text-berry-700 dark:bg-berry-900 dark:text-rose-100',
                )
              }
            >
              <Gift className="size-4" aria-hidden="true" />
              <span>{boxes > 0 ? `${boxes > 9 ? '9+' : boxes} to open` : 'Locker'}</span>
              <UserAvatar
                name={data?.user.display_name ?? 'Coffee friend'}
                artworkPath={avatarArtwork}
                frameId={locker?.equipped.frame}
                size="sm"
              />
            </NavLink>
            <div className="hidden items-center gap-3 md:flex">
              <SeasonBadge />
              <NavLink
                to="/locker"
                className="flex items-center gap-2 rounded-full bg-white py-1 pl-1 pr-4 text-sm font-bold text-berry-700 shadow-soft transition hover:ring-2 hover:ring-primary/20 dark:bg-berry-900 dark:text-rose-100"
              >
                <UserAvatar
                  name={data?.user.display_name ?? 'Coffee friend'}
                  artworkPath={avatarArtwork}
                  frameId={locker?.equipped.frame}
                  size="sm"
                />
                <span className="flex flex-col leading-tight">
                  <span>{data?.user.display_name ?? 'Coffee friend'}</span>
                  {equippedTitle ? (
                    <span className="text-[0.65rem] font-black italic text-primary dark:text-blush">
                      {equippedTitle}
                    </span>
                  ) : null}
                </span>
              </NavLink>
              <Button variant="ghost" onClick={logout} aria-label="Log out">
                <LogOut className="size-4" />
                Logout
              </Button>
            </div>
          </>}
        </div>
      </header>

      {simple && (
        <dialog
          ref={menu}
          onClose={() => document.getElementById('navigation-trigger')?.focus()}
          id="navigation-menu"
          aria-labelledby="navigation-title"
          className="m-auto max-h-[85svh] w-[calc(100%_-_2rem)] max-w-md overflow-y-auto rounded-3xl border border-berry-100 bg-card p-5 text-berry-950 shadow-soft backdrop:bg-black/35 dark:border-white/10 dark:bg-berry-900 dark:text-berry-50"
          onClick={(event) => {
            if (event.target === event.currentTarget) menu.current?.close()
          }}
        >
          <div className="mb-3 flex items-center justify-between">
            <h2 id="navigation-title" className="font-display text-xl font-black">Menu</h2>
            <Button variant="ghost" aria-label="Close menu" onClick={() => menu.current?.close()}><X className="size-5" /></Button>
          </div>
          <nav aria-label="Main navigation" className="grid gap-1">
            {simpleNavItems.map(({ to, label, icon: Icon }) => (
              <NavLink key={to} to={to} onClick={() => menu.current?.close()} className={({ isActive }) => cn('flex min-h-12 items-center gap-3 rounded-2xl px-4 text-sm font-bold transition', isActive ? 'bg-primary text-white dark:bg-blush dark:text-berry-950' : 'hover:bg-primary/10 dark:hover:bg-white/10')}>
                <Icon className="size-5" aria-hidden="true" />{label}
              </NavLink>
            ))}
          </nav>
          <div className="mt-3 border-t border-primary/10 pt-3">
            <Button variant="ghost" onClick={logout}><LogOut className="size-4" /> Logout</Button>
          </div>
        </dialog>
      )}
      <div className={cn('relative z-10 mx-auto grid gap-6 px-4 py-5 sm:px-6 md:py-8', simple ? 'max-w-3xl' : 'max-w-7xl md:grid-cols-[13rem_1fr]')}>
        {!simple && <nav
          aria-label="Main navigation"
          className="fixed inset-x-3 bottom-3 z-40 grid rounded-4xl border-2 border-white/80 bg-white/90 p-2 shadow-soft backdrop-blur md:sticky md:top-24 md:inset-auto md:block md:h-fit md:space-y-2 md:bg-white/70 dark:border-white/10 dark:bg-berry-900/92"
          style={{
            gridTemplateColumns: `repeat(${navItems.filter((item) => !item.desktopOnly).length}, minmax(0, 1fr))`,
          }}
        >
          {navItems.map((item) => {
            const Icon = item.icon
            return (
              <NavLink
                key={item.to}
                to={item.to}
                className={({ isActive }) =>
                  cn(
                    'relative flex min-h-12 flex-col items-center justify-center gap-0.5 rounded-3xl px-0.5 text-center text-[0.68rem] font-black leading-tight text-berry-700 transition sm:min-h-14 sm:gap-1 sm:px-2 sm:text-xs md:min-h-12 md:flex-row md:justify-start md:px-4 md:text-left md:text-sm',
                    isActive
                      ? 'bg-primary text-white shadow-button dark:bg-blush dark:text-berry-950'
                      : 'hover:bg-primary/10 dark:text-rose-100 dark:hover:bg-white/10',
                    item.desktopOnly && 'hidden md:flex',
                  )
                }
              >
                <Icon className="size-4 sm:size-5" aria-hidden="true" />
                <span className="md:hidden">{item.mobileLabel ?? item.label}</span>
                <span className="hidden md:inline">{item.label}</span>
                {item.badge ? (
                  <span className="ml-auto hidden rounded-full bg-sun px-2 py-0.5 text-xs font-black text-berry-950 md:inline">
                    {item.badge}
                  </span>
                ) : null}
              </NavLink>
            )
          })}
        </nav>}
        <main className="min-w-0">
          {simple && hiddenInSimpleView ? <Navigate to="/" replace /> : <Outlet />}
          <AppFooter compact={simple && location.pathname !== '/settings'} />
        </main>
        {!simple && <CrateReminder />}
      </div>
    </div>
  )
}
