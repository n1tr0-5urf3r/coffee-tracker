import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react'

export type ThemeChoice = 'system' | 'light' | 'dark'

type ThemeContextValue = {
  theme: ThemeChoice
  resolvedTheme: 'light' | 'dark'
  setTheme: (theme: ThemeChoice) => void
}

const ThemeContext = createContext<ThemeContextValue | null>(null)

function getSystemTheme() {
  return window.matchMedia('(prefers-color-scheme: dark)').matches
    ? 'dark'
    : 'light'
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<ThemeChoice>(() => {
    return (localStorage.getItem('coffee-theme') as ThemeChoice | null) ?? 'system'
  })
  const [systemTheme, setSystemTheme] = useState<'light' | 'dark'>(() =>
    getSystemTheme(),
  )

  useEffect(() => {
    const media = window.matchMedia('(prefers-color-scheme: dark)')
    const listener = () => setSystemTheme(getSystemTheme())
    media.addEventListener('change', listener)
    return () => media.removeEventListener('change', listener)
  }, [])

  const resolvedTheme = theme === 'system' ? systemTheme : theme

  useEffect(() => {
    document.documentElement.dataset.theme = resolvedTheme
    document.documentElement.classList.toggle('dark', resolvedTheme === 'dark')
  }, [resolvedTheme])

  const value = useMemo(
    () => ({
      theme,
      resolvedTheme,
      setTheme: (nextTheme: ThemeChoice) => {
        localStorage.setItem('coffee-theme', nextTheme)
        setThemeState(nextTheme)
      },
    }),
    [theme, resolvedTheme],
  )

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>
}

export function useTheme() {
  const context = useContext(ThemeContext)
  if (!context) {
    throw new Error('useTheme must be used within ThemeProvider')
  }
  return context
}
