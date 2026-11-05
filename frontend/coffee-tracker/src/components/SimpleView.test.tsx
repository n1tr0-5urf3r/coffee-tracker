import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { AppShell } from './AppShell'
import { LogPage } from '../pages/LogPage'
import { SimpleViewSwitch } from './SimpleViewSwitch'
import meFixture from '../../fixtures/me.json'
import { ViewModeProvider } from '../lib/viewMode'
import { api } from '../api/client'
import catalog from '../../fixtures/beverages.json'

const state = vi.hoisted(() => ({ member: true, freeDrinks: 0, simple: false, pushToast: vi.fn() }))
vi.mock('../api/queries', async (importOriginal) => ({
  ...await importOriginal<typeof import('../api/queries')>(),
  useLocker: () => ({ data: { items: [], equipped: {}, boxes_available: 3 } }),
  useWheel: () => ({ data: { free_spins_available: 2, free_drinks_available: state.freeDrinks } }),
  useCatalog: () => ({ machines: { data: { machines: [{ id: 'main', name: 'Office' }] } }, beverages: { data: catalog, isLoading: false, isSuccess: true } }),
  useStats: () => ({ summary: { data: { recent_consumptions: [] } } }),
  useStampCard: () => ({}),
  invalidateCoffeeData: vi.fn(),
}))
vi.mock('../lib/toast', () => ({ useToast: () => ({ pushToast: state.pushToast }) }))
vi.mock('../lib/seasonalTheme', () => ({ useSeasonalTheme: () => ({ activeSeason: null }) }))
vi.mock('../lib/offlineQueue', () => ({ queueLog: vi.fn() }))
vi.mock('../hooks/useOfflineQueue', () => ({ useOfflineQueue: () => ({ queuedLogs: [], refreshQueue: vi.fn(), cancelQueuedLog: vi.fn() }) }))
vi.mock('./SeasonalDecor', () => ({ SeasonalDecor: () => <div>Seasonal decorations</div>, SeasonBadge: () => null }))
vi.mock('./AppFooter', () => ({ AppFooter: () => null }))
vi.mock('./CrateReminder', () => ({ CrateReminder: () => <div>Reward reminder</div> }))
vi.mock('./ChoresCard', () => ({ ChoresCard: () => <div>Clean the machine</div> }))
vi.mock('./WheelPill', () => ({ WheelPill: () => <div>Wheel promotion</div> }))

function meResponse() {
  return { user: { ...meFixture.user, id: 'test', user_type: state.member ? 'member' : 'guest', simple_view: state.simple, paypal_link: null } }
}

function mount(path = '/') {
  return render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <ViewModeProvider>
        <MemoryRouter initialEntries={[path]}>
          <Routes>
            <Route element={<AppShell />}>
              <Route index element={<LogPage />} />
              <Route path="/settings" element={<SimpleViewSwitch />} />
              <Route path="/locker" element={<h1>Locker page</h1>} />
              <Route path="/payments" element={<h1>Payments page</h1>} />
            </Route>
          </Routes>
        </MemoryRouter>
      </ViewModeProvider>
    </QueryClientProvider>,
  )
}

beforeEach(() => {
  state.member = true
  state.simple = false
  vi.spyOn(api, 'me').mockImplementation(async () => meResponse())
  vi.spyOn(api, 'updateSettings').mockImplementation(async (body) => {
    state.simple = body.simple_view ?? state.simple
    return meResponse()
  })
  state.freeDrinks = 0
  state.pushToast.mockClear()
  // jsdom does not implement the browser's native dialog methods.
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute('open', '') }
  HTMLDialogElement.prototype.close = function () { this.removeAttribute('open') }
})
afterEach(() => { cleanup(); vi.restoreAllMocks() })

describe('Simple view', () => {
  it('saves the Settings switch per account, restores it on another device, and can disable it', async () => {
    const user = userEvent.setup()
    const first = mount('/settings')
    const toggle = await screen.findByRole('switch', { name: 'Simple view' })
    expect(toggle).toHaveAttribute('aria-checked', 'false')
    await user.click(toggle)
    await waitFor(() => expect(toggle).toHaveAttribute('aria-checked', 'true'))
    expect(api.updateSettings).toHaveBeenCalledWith({ simple_view: true })
    expect(screen.queryByRole('navigation')).not.toBeInTheDocument()
    first.unmount()
    // A fresh query client represents a new browser, without local preferences.
    mount('/settings')
    const restored = await screen.findByRole('switch', { name: 'Simple view' })
    await waitFor(() => expect(restored).toHaveAttribute('aria-checked', 'true'))
    await user.click(restored)
    await waitFor(() => expect(restored).toHaveAttribute('aria-checked', 'false'))
    expect(screen.getByRole('navigation')).toBeVisible()
  })

  it('leaves the saved preference unchanged when saving fails and reports the error', async () => {
    vi.mocked(api.updateSettings).mockRejectedValue(new Error('offline'))
    mount('/settings')
    await userEvent.click(await screen.findByRole('switch', { name: 'Simple view' }))
    await waitFor(() => expect(state.pushToast).toHaveBeenCalledWith(expect.objectContaining({ tone: 'error' })))
    expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'false')
  })

  it('shows only essential destinations in Menu and closes after selecting or dismissing', async () => {
    state.simple = true
    const user = userEvent.setup()
    mount()
    await user.click(await screen.findByRole('button', { name: 'Menu' }))
    const dialog = screen.getByRole('dialog', { name: 'Menu' })
    for (const name of ['Log', 'Stats', 'Payments', 'Settings']) expect(within(dialog).getByRole('link', { name })).toBeVisible()
    for (const name of ['Locker', 'Lucky wheel', 'Badges', 'Stempelkarte', 'Community Ranking']) expect(within(dialog).queryByRole('link', { name })).not.toBeInTheDocument()
    await user.click(within(dialog).getByRole('button', { name: 'Close menu' }))
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    await user.click(await screen.findByRole('button', { name: 'Menu' }))
    await user.click(screen.getByRole('link', { name: 'Payments' }))
    expect(screen.getByRole('heading', { name: 'Payments page' })).toBeVisible()
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(screen.queryByRole('navigation')).not.toBeInTheDocument()
  })

  it('hides nonessential routes in Simple view and restores them in full view', async () => {
    state.simple = true
    const first = mount('/locker')
    expect(await screen.findByRole('heading', { name: 'Log a drink' })).toBeVisible()
    expect(screen.queryByRole('heading', { name: 'Locker page' })).not.toBeInTheDocument()
    first.unmount()
    state.simple = false
    mount('/locker')
    expect(await screen.findByRole('heading', { name: 'Locker page' })).toBeVisible()
  })

  it('keeps guest navigation appropriate to their account', async () => {
    state.member = false
    state.simple = true
    mount()
    await userEvent.click(await screen.findByRole('button', { name: 'Menu' }))
    expect(screen.queryByRole('link', { name: 'Lucky wheel' })).not.toBeInTheDocument()
    expect(screen.queryByRole('link', { name: 'Locker' })).not.toBeInTheDocument()
  })

  it('logs every drink with the existing API and preserves undo without reward prompts', async () => {
    state.simple = true
    const create = vi.spyOn(api, 'createConsumption').mockResolvedValue({ id: 'log-1', user_id: 'test', machine_id: 'main', beverage_id: 'tea', consumed_at: Math.floor(Date.now() / 1000), crates_earned: ['daily'], free_drink_voucher_used: false })
    const undo = vi.spyOn(api, 'deleteConsumption').mockResolvedValue(undefined)
    mount()
    await screen.findByRole('heading', { name: 'Log a drink' })
    expect(screen.queryByText('Reward reminder')).not.toBeInTheDocument()
    expect(screen.queryByText('Seasonal decorations')).not.toBeInTheDocument()
    expect(screen.queryByText('Wheel promotion')).not.toBeInTheDocument()
    for (const beverage of catalog.beverages) expect(screen.getByRole('button', { name: `Log ${beverage.name}` })).toBeVisible()
    expect(screen.getByText('Clean the machine')).not.toBeVisible()
    await userEvent.click(screen.getByText('Machine chores'))
    expect(screen.getByText('Clean the machine')).toBeVisible()
    await userEvent.click(screen.getByRole('button', { name: 'Log Tea' }))
    await waitFor(() => expect(create).toHaveBeenCalledWith({ machine_id: 'main', beverage_id: 'tea' }))
    await userEvent.click(await screen.findByRole('button', { name: /Undo tap/ }))
    await waitFor(() => expect(undo).toHaveBeenCalledWith('log-1'))
    expect(state.pushToast.mock.calls.some(([toast]) => toast.title.includes('crate'))).toBe(false)
  })

  it('shows free-drink pricing when a voucher is available', async () => {
    state.simple = true
    state.freeDrinks = 1
    mount()
    expect(await screen.findByText('Your wheel voucher covers drinks up to 0.50 €.')).toBeVisible()
    expect(within(screen.getByRole('button', { name: 'Log Tea' })).getByText('Free')).toBeVisible()
    expect(within(screen.getByRole('button', { name: 'Log Other' })).queryByText('Free')).not.toBeInTheDocument()
  })
})
