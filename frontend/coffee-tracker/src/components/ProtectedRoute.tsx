import { Navigate, Outlet, useLocation } from 'react-router-dom'
import { isBackendUnavailableError } from '../api/client'
import { useMe } from '../api/queries'
import { LogoMark } from './LogoMark'

export function ProtectedRoute() {
  const location = useLocation()
  const { isLoading, isError, error } = useMe()

  if (isLoading) {
    return (
      <div className="grid min-h-svh place-items-center p-6">
        <div className="rounded-4xl bg-white/80 p-8 shadow-soft dark:bg-berry-900">
          <LogoMark className="animate-pulse" />
          <p className="mt-5 font-bold text-berry-700 dark:text-rose-100">
            Warming up the counter...
          </p>
        </div>
      </div>
    )
  }

  if (isError && !isBackendUnavailableError(error)) {
    return <Navigate to="/login" state={{ from: location }} replace />
  }

  return <Outlet />
}
