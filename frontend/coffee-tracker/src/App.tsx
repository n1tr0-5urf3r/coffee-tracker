import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import {
  Navigate,
  RouterProvider,
  createBrowserRouter,
} from 'react-router-dom'
import { AppShell } from './components/AppShell'
import { BackendHealthBoundary } from './components/BackendStatus'
import { ProtectedRoute } from './components/ProtectedRoute'
import { AchievementsPage } from './pages/AchievementsPage'
import { ComponentsPage } from './pages/ComponentsPage'
import { DashboardPage } from './pages/DashboardPage'
import { LeaderboardPage } from './pages/LeaderboardPage'
import { LoginPage } from './pages/LoginPage'
import { LogPage } from './pages/LogPage'
import { PaymentsPage } from './pages/PaymentsPage'
import { SettingsPage } from './pages/SettingsPage'
import { StempelkartePage } from './pages/StempelkartePage'
import { ThemeProvider } from './lib/theme'
import { ToastProvider } from './lib/toast'

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 20_000,
      retry: (failureCount, error) => {
        if (error instanceof Error && error.name === 'ApiError') {
          return false
        }
        return failureCount < 1
      },
    },
  },
})

const router = createBrowserRouter([
  { path: '/login', element: <LoginPage /> },
  {
    element: <ProtectedRoute />,
    children: [
      {
        element: <AppShell />,
        children: [
          { index: true, element: <LogPage /> },
          { path: '/stempelkarte', element: <StempelkartePage /> },
          { path: '/dashboard', element: <DashboardPage /> },
          { path: '/achievements', element: <AchievementsPage /> },
          { path: '/leaderboard', element: <LeaderboardPage /> },
          { path: '/payments', element: <PaymentsPage /> },
          { path: '/settings', element: <SettingsPage /> },
          { path: '/components', element: <ComponentsPage /> },
        ],
      },
    ],
  },
  { path: '*', element: <Navigate to="/" replace /> },
])

function App() {
  return (
    <ThemeProvider>
      <QueryClientProvider client={queryClient}>
        <ToastProvider>
          <BackendHealthBoundary>
            <RouterProvider router={router} />
          </BackendHealthBoundary>
        </ToastProvider>
      </QueryClientProvider>
    </ThemeProvider>
  )
}

export default App
