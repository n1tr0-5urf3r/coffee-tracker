import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from 'react'
import { AnimatePresence, motion } from 'framer-motion'
import { CheckCircle2, Info, TriangleAlert } from 'lucide-react'
import { cn } from './cn'

type ToastTone = 'success' | 'info' | 'error'

type Toast = {
  id: string
  title: string
  description?: string
  tone: ToastTone
}

type ToastContextValue = {
  pushToast: (toast: Omit<Toast, 'id'>) => void
}

const ToastContext = createContext<ToastContextValue | null>(null)

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([])

  const pushToast = useCallback((toast: Omit<Toast, 'id'>) => {
    const id = crypto.randomUUID()
    setToasts((current) => [...current, { ...toast, id }])
    window.setTimeout(() => {
      setToasts((current) => current.filter((item) => item.id !== id))
    }, 4200)
  }, [])

  const value = useMemo(() => ({ pushToast }), [pushToast])

  return (
    <ToastContext.Provider value={value}>
      {children}
      <div
        aria-live="polite"
        className="fixed inset-x-4 top-4 z-50 flex flex-col gap-3 sm:left-auto sm:w-96"
      >
        <AnimatePresence>
          {toasts.map((toast) => (
            <motion.div
              key={toast.id}
              layout
              initial={{ opacity: 0, y: -18, scale: 0.96 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: -12, scale: 0.96 }}
              className={cn(
                'rounded-3xl border-2 bg-white/95 p-4 shadow-soft backdrop-blur dark:bg-berry-950/95',
                toast.tone === 'success' && 'border-mint text-berry-950',
                toast.tone === 'info' && 'border-blush text-berry-950',
                toast.tone === 'error' && 'border-primary text-berry-950',
                'dark:text-rose-50',
              )}
            >
              <div className="flex gap-3">
                {toast.tone === 'success' ? (
                  <CheckCircle2 className="mt-0.5 size-5 text-mint" />
                ) : toast.tone === 'error' ? (
                  <TriangleAlert className="mt-0.5 size-5 text-primary" />
                ) : (
                  <Info className="mt-0.5 size-5 text-secondary" />
                )}
                <div>
                  <p className="font-display text-base font-black">
                    {toast.title}
                  </p>
                  {toast.description ? (
                    <p className="mt-1 text-sm text-berry-700 dark:text-rose-100">
                      {toast.description}
                    </p>
                  ) : null}
                </div>
              </div>
            </motion.div>
          ))}
        </AnimatePresence>
      </div>
    </ToastContext.Provider>
  )
}

export function useToast() {
  const context = useContext(ToastContext)
  if (!context) {
    throw new Error('useToast must be used within ToastProvider')
  }
  return context
}
