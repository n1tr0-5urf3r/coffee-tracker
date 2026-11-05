import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { cleanup, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '../api/client'
import { todayKey } from '../lib/stats'
import summary from '../../fixtures/stats-summary.json'
import meFixture from '../../fixtures/me.json'
import { SimpleStatsPage } from './SimpleStatsPage'
import { SimplePaymentsPage } from './SimplePaymentsPage'

function mount(page: React.ReactNode) {
  return render(<QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>{page}</QueryClientProvider>)
}

afterEach(() => { cleanup(); vi.restoreAllMocks() })

describe('Simple stats', () => {
  it('shows personal drink counts without fetching global stats', async () => {
    vi.spyOn(api, 'summary').mockResolvedValue(summary)
    vi.spyOn(api, 'buckets').mockResolvedValue({ buckets: [{ date: todayKey(), count: 3, price_cents: 150 }] })
    const global = vi.spyOn(api, 'globalStats')
    mount(<SimpleStatsPage />)
    expect(await screen.findByText('All time')).toBeVisible()
    expect(screen.getByText('Today').parentElement).toHaveTextContent('3')
    expect(screen.getByText('Last 7 days').parentElement).toHaveTextContent('3')
    expect(screen.getByText('All time').parentElement).toHaveTextContent('138')
    expect(global).not.toHaveBeenCalled()
    expect(screen.queryByText('Caffeine curve')).not.toBeInTheDocument()
  })

  it('reports failed counts and retries instead of showing zero', async () => {
    vi.spyOn(api, 'summary').mockResolvedValue(summary)
    vi.spyOn(api, 'buckets').mockRejectedValueOnce(new Error('offline')).mockResolvedValueOnce({ buckets: [] })
    mount(<SimpleStatsPage />)
    const error = await screen.findByRole('alert')
    expect(error).toHaveTextContent('Recent drink totals could not be loaded')
    await userEvent.click(within(error).getByRole('button', { name: 'Try again' }))
    expect(await screen.findByText('Today')).toBeVisible()
  })
})

describe('Simple payments', () => {
  beforeEach(() => {
    vi.spyOn(api, 'me').mockResolvedValue({ user: { ...meFixture.user, paypal_link: null } })
    vi.spyOn(api, 'paymentRecipients').mockResolvedValue({ entries: [
      { user_id: 'staff-1', display_name: 'Ada', paypal_link: 'https://paypal.me/ada' },
      { user_id: 'staff-2', display_name: 'Bea', paypal_link: 'https://paypal.me/bea' },
    ] })
  })

  it('lets the user pay the correct amount and confirm their own tab with a chosen recipient', async () => {
    const user = userEvent.setup()
    const debt = { user_id: 'mine', unpaid_amount_cents: 1800, amount_due_cents: 1300, unpaid_wheel_cents: 0, unpaid_drink_count: 36, paid_amount_cents: 0, paid_drink_count: 0, expense_amount_cents: 500, received_payment_amount_cents: 0, expense_balance_cents: 500 }
    vi.spyOn(api, 'myPayment').mockResolvedValue(debt)
    const markPaid = vi.spyOn(api, 'markPaid').mockResolvedValue({ id: 'payment', user_id: 'mine', received_by_user_id: 'staff-2', amount_cents: 1800, expense_settled_cents: -500, status: 'pending', drink_count: 36, paid_at: 1, period_end_ts: 1 })
    mount(<SimplePaymentsPage />)
    const payTo = await screen.findByRole('combobox', { name: 'Pay to' })
    expect(payTo).toHaveValue('')
    expect(screen.queryByRole('link', { name: 'Pay online' })).not.toBeInTheDocument()
    await user.selectOptions(payTo, 'staff-1')
    expect(screen.getByRole('link', { name: 'Pay online' })).toHaveAttribute('href', 'https://paypal.me/ada/13.00')
    await user.selectOptions(payTo, 'staff-2')
    expect(screen.getByRole('link', { name: 'Pay online' })).toHaveAttribute('href', 'https://paypal.me/bea/13.00')
    await user.click(screen.getByRole('button', { name: 'Copy payment link' }))
    expect(await navigator.clipboard.readText()).toBe('https://paypal.me/bea/13.00')
    await user.click(screen.getByRole('button', { name: 'Mark paid' }))
    expect(screen.getByText(/Submit your tab/)).toHaveTextContent('13,00')
    const confirm = screen.getByRole('button', { name: 'Confirm payment' })
    expect(screen.getAllByRole('combobox')).toHaveLength(1)
    expect(screen.getByText('Confirm payment to Bea')).toBeVisible()
    await user.click(confirm)
    expect(await screen.findByText('Payment submitted; awaiting recipient confirmation.')).toBeVisible()
    expect(markPaid).toHaveBeenCalledWith(expect.objectContaining({ user_id: 'mine', received_by_user_id: 'staff-2' }))
  })

  it('uses the amount due returned by the server for the displayed balance and link', async () => {
    vi.spyOn(api, 'myPayment').mockResolvedValue({ user_id: 'mine', unpaid_amount_cents: 500, amount_due_cents: 900, unpaid_wheel_cents: 0, unpaid_drink_count: 10, paid_amount_cents: 0, paid_drink_count: 0, expense_amount_cents: 0, received_payment_amount_cents: 0, expense_balance_cents: 0 })
    mount(<SimplePaymentsPage />)
    expect(await screen.findByText(/9,00/)).toBeVisible()
    await userEvent.selectOptions(await screen.findByRole('combobox', { name: 'Pay to' }), 'staff-1')
    expect(screen.getByRole('link', { name: 'Pay online' })).toHaveAttribute('href', 'https://paypal.me/ada/9.00')
  })

  it('shows the pending payment and prevents submitting a second claim', async () => {
    vi.spyOn(api, 'myPayment').mockResolvedValue({ user_id: 'mine', unpaid_amount_cents: 200, amount_due_cents: 200, unpaid_wheel_cents: 0, unpaid_drink_count: 4, paid_amount_cents: 0, paid_drink_count: 0, expense_amount_cents: 0, received_payment_amount_cents: 0, expense_balance_cents: 0, pending_payment: { id: 'pending', user_id: 'mine', received_by_user_id: 'staff-1', amount_cents: 500, expense_settled_cents: -100, status: 'pending', drink_count: 10, paid_at: 1, period_end_ts: 1 } })
    mount(<SimplePaymentsPage />)
    expect(await screen.findByText(/is waiting for/)).toHaveTextContent('4,00')
    expect(screen.queryByRole('button', { name: 'Mark paid' })).not.toBeInTheDocument()
  })

  it('lets staff confirm or reject incoming payments without exposing other users’ balances', async () => {
    vi.mocked(api.me).mockResolvedValue({ user: { ...meFixture.user, paypal_link: null, stock_manager: true } })
    vi.spyOn(api, 'myPayment').mockResolvedValue({ user_id: 'staff', unpaid_amount_cents: 0, amount_due_cents: 0, unpaid_wheel_cents: 0, unpaid_drink_count: 0, paid_amount_cents: 0, paid_drink_count: 0, expense_amount_cents: 0, received_payment_amount_cents: 0, expense_balance_cents: 0 })
    vi.spyOn(api, 'payments').mockResolvedValue({ entries: [], received_payments: [{ id: 'pending', payer_user_id: 'payer', payer_display_name: 'Pat', recipient_user_id: 'staff', recipient_display_name: 'Ada', amount_cents: 500, expense_settled_cents: -100, drink_count: 10, paid_at: 1, status: 'pending' }] })
    const confirm = vi.spyOn(api, 'confirmPayment').mockResolvedValue({ id: 'pending', user_id: 'payer', received_by_user_id: 'staff', amount_cents: 500, expense_settled_cents: -100, drink_count: 10, paid_at: 1, period_end_ts: 1, status: 'confirmed' })
    mount(<SimplePaymentsPage />)
    expect(await screen.findByText(/Pat ·/)).toHaveTextContent('4,00')
    await userEvent.click(screen.getByRole('button', { name: 'Confirm' }))
    expect(confirm).toHaveBeenCalledWith('pending')
    expect(screen.queryByText('Payment tracking')).not.toBeInTheDocument()
  })

  it('requires a new confirmation if the recipient changes', async () => {
    vi.spyOn(api, 'myPayment').mockResolvedValue({ user_id: 'mine', unpaid_amount_cents: 200, amount_due_cents: 200, unpaid_wheel_cents: 0, unpaid_drink_count: 4, paid_amount_cents: 0, paid_drink_count: 0, expense_amount_cents: 0, received_payment_amount_cents: 0, expense_balance_cents: 0 })
    const user = userEvent.setup()
    mount(<SimplePaymentsPage />)
    const payTo = await screen.findByRole('combobox', { name: 'Pay to' })
    await user.selectOptions(payTo, 'staff-1')
    await user.click(screen.getByRole('button', { name: 'Mark paid' }))
    expect(screen.getByText('Confirm payment to Ada')).toBeVisible()
    await user.selectOptions(payTo, 'staff-2')
    expect(screen.queryByRole('button', { name: 'Confirm payment' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Mark paid' }))
    expect(screen.getByText('Confirm payment to Bea')).toBeVisible()
  })

  it('supports recording a cash payment to a recipient without an online link', async () => {
    vi.mocked(api.paymentRecipients).mockResolvedValue({ entries: [{ user_id: 'staff-1', display_name: 'Ada', paypal_link: null }] })
    vi.spyOn(api, 'myPayment').mockResolvedValue({ user_id: 'mine', unpaid_amount_cents: 200, amount_due_cents: 200, unpaid_wheel_cents: 0, unpaid_drink_count: 4, paid_amount_cents: 0, paid_drink_count: 0, expense_amount_cents: 0, received_payment_amount_cents: 0, expense_balance_cents: 0 })
    mount(<SimplePaymentsPage />)
    await userEvent.selectOptions(await screen.findByRole('combobox', { name: 'Pay to' }), 'staff-1')
    expect(screen.getByText(/No online payment link for Ada/)).toBeVisible()
    await userEvent.click(screen.getByRole('button', { name: 'Mark paid' }))
    expect(screen.getByText('Confirm payment to Ada')).toBeVisible()
  })

  it('allows settling an unpaid tab even if expense credit makes the amount due zero', async () => {
    vi.spyOn(api, 'myPayment').mockResolvedValue({ user_id: 'mine', unpaid_amount_cents: 200, amount_due_cents: 0, unpaid_wheel_cents: 0, unpaid_drink_count: 4, paid_amount_cents: 0, paid_drink_count: 0, expense_amount_cents: 300, received_payment_amount_cents: 0, expense_balance_cents: 300 })
    mount(<SimplePaymentsPage />)
    await userEvent.selectOptions(await screen.findByRole('combobox', { name: 'Pay to' }), 'staff-1')
    expect(screen.getByRole('button', { name: 'Mark paid' })).toBeVisible()
    expect(screen.queryByRole('link', { name: 'Pay online' })).not.toBeInTheDocument()
  })

  it('preserves the confirmation on failure so the user can retry', async () => {
    vi.spyOn(api, 'myPayment').mockResolvedValue({ user_id: 'mine', unpaid_amount_cents: 200, amount_due_cents: 200, unpaid_wheel_cents: 0, unpaid_drink_count: 4, paid_amount_cents: 0, paid_drink_count: 0, expense_amount_cents: 0, received_payment_amount_cents: 0, expense_balance_cents: 0 })
    vi.spyOn(api, 'markPaid').mockRejectedValue(new Error('offline'))
    mount(<SimplePaymentsPage />)
    await userEvent.selectOptions(await screen.findByRole('combobox', { name: 'Pay to' }), 'staff-1')
    await userEvent.click(screen.getByRole('button', { name: 'Mark paid' }))
    await userEvent.click(screen.getByRole('button', { name: 'Confirm payment' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('Payment could not be recorded')
    expect(screen.getByRole('button', { name: 'Confirm payment' })).toBeEnabled()
  })

  it('shows only the signed-in user’s amount due after expense credits', async () => {
    vi.spyOn(api, 'myPayment').mockResolvedValue({ user_id: 'mine', unpaid_amount_cents: 1800, amount_due_cents: 1300, unpaid_wheel_cents: 0, unpaid_drink_count: 36, paid_amount_cents: 0, paid_drink_count: 0, expense_amount_cents: 500, received_payment_amount_cents: 0, expense_balance_cents: 500 })
    const allPayments = vi.spyOn(api, 'payments')
    mount(<SimplePaymentsPage />)
    expect(await screen.findByText('Your amount due')).toBeVisible()
    expect(screen.getByText(/13,00/)).toBeVisible()
    expect(allPayments).not.toHaveBeenCalled()
    expect(screen.queryByText('Payment tracking')).not.toBeInTheDocument()
  })

  it('reports failure and allows a retry', async () => {
    vi.spyOn(api, 'myPayment').mockRejectedValueOnce(new Error('offline')).mockResolvedValueOnce({ user_id: 'mine', unpaid_amount_cents: 0, amount_due_cents: 0, unpaid_wheel_cents: 0, unpaid_drink_count: 0, paid_amount_cents: 0, paid_drink_count: 0, expense_amount_cents: 0, received_payment_amount_cents: 0, expense_balance_cents: 0 })
    mount(<SimplePaymentsPage />)
    const error = await screen.findByRole('alert')
    await userEvent.click(within(error).getByRole('button', { name: 'Try again' }))
    expect(await screen.findByText('Your amount due')).toBeVisible()
    expect(screen.getByText(/0,00/)).toBeVisible()
  })
})
