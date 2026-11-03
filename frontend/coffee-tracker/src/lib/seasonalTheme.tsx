import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react'
import { parseSeasonOverride, resolveSeason, type SeasonId } from './seasons'

const STORAGE_KEY = 'coffee-seasonal'
const RECHECK_INTERVAL_MS = 60 * 60 * 1000

type SeasonalThemeContextValue = {
  enabled: boolean
  setEnabled: (enabled: boolean) => void
  /** Season for today (or the `?season=` preview), regardless of the user's choice. */
  currentSeason: SeasonId
  /** Season that is actually displayed; `null` when the user turned seasonal themes off. */
  activeSeason: SeasonId | null
}

const SeasonalThemeContext = createContext<SeasonalThemeContextValue | null>(null)

function readEnabled() {
  try {
    return localStorage.getItem(STORAGE_KEY) !== 'off'
  } catch {
    return true
  }
}

function computeSeason(): SeasonId {
  return parseSeasonOverride(window.location.search) ?? resolveSeason(new Date())
}

export function SeasonalThemeProvider({ children }: { children: ReactNode }) {
  const [enabled, setEnabledState] = useState(readEnabled)
  const [currentSeason, setCurrentSeason] = useState<SeasonId>(computeSeason)

  // Keep long-running tabs (e.g. a tablet next to the machine) in sync with the calendar.
  useEffect(() => {
    const recheck = () => setCurrentSeason(computeSeason())
    const interval = window.setInterval(recheck, RECHECK_INTERVAL_MS)
    document.addEventListener('visibilitychange', recheck)
    return () => {
      window.clearInterval(interval)
      document.removeEventListener('visibilitychange', recheck)
    }
  }, [])

  const activeSeason = enabled ? currentSeason : null

  useEffect(() => {
    const root = document.documentElement
    if (activeSeason) {
      root.dataset.season = activeSeason
    } else {
      delete root.dataset.season
    }
  }, [activeSeason])

  const value = useMemo(
    () => ({
      enabled,
      currentSeason,
      activeSeason,
      setEnabled: (next: boolean) => {
        try {
          localStorage.setItem(STORAGE_KEY, next ? 'on' : 'off')
        } catch {
          // Storage may be unavailable (private mode); the choice still applies for this session.
        }
        setEnabledState(next)
      },
    }),
    [enabled, currentSeason, activeSeason],
  )

  return (
    <SeasonalThemeContext.Provider value={value}>{children}</SeasonalThemeContext.Provider>
  )
}

export function useSeasonalTheme() {
  const context = useContext(SeasonalThemeContext)
  if (!context) {
    throw new Error('useSeasonalTheme must be used within SeasonalThemeProvider')
  }
  return context
}
