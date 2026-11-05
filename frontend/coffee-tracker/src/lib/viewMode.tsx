import { createContext, useContext, type ReactNode } from 'react'
import { useMe, useSettingsMutation } from '../api/queries'
import { useToast } from './toast'

const ViewModeContext = createContext<{
  simple: boolean
  saving: boolean
  setSimple: (simple: boolean) => void
} | null>(null)

export function ViewModeProvider({ children }: { children: ReactNode }) {
  const { data } = useMe()
  const settings = useSettingsMutation()
  const { pushToast } = useToast()
  return (
    <ViewModeContext.Provider value={{
      simple: data?.user.simple_view ?? false,
      saving: settings.isPending,
      setSimple: (simple) => settings.mutate({ simple_view: simple }, {
        onError: () => pushToast({
          tone: 'error',
          title: 'View preference could not be saved',
          description: 'Please try again when you are connected.',
        }),
      }),
    }}>
      {children}
    </ViewModeContext.Provider>
  )
}

export function useViewMode() {
  const context = useContext(ViewModeContext)
  if (!context) throw new Error('useViewMode must be used within ViewModeProvider')
  return context
}
