import { createElement, useState } from 'react'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { PaymentSummary } from '../api/client'
import {
  autoReminderNote,
  buildPaymentUrl,
  EmailReminderDialog,
  resolvePaymentsSection,
} from './PaymentsPage'

afterEach(cleanup)

describe('resolvePaymentsSection', () => {
  it('opens staff-only sections for staff', () => {
    expect(resolvePaymentsSection('expenses', true)).toBe('expenses')
    expect(resolvePaymentsSection('ledger', true)).toBe('ledger')
  })

  it('falls back to the overview for non-staff and unknown sections', () => {
    expect(resolvePaymentsSection('expenses', false)).toBe('overview')
    expect(resolvePaymentsSection('ledger', false)).toBe('overview')
    expect(resolvePaymentsSection('nope', true)).toBe('overview')
    expect(resolvePaymentsSection(undefined, true)).toBe('overview')
  })
})

describe('autoReminderNote', () => {
  const links = [{ user_id: 'staff', display_name: 'Etienne Zink', paypal_link: 'https://paypal.me/x' }]

  it('names the configured payment link owner and a zero-padded hour', () => {
    expect(autoReminderNote({ payment_link_user_id: 'staff', hour: 9 }, links)).toBe(
      "Sent automatically on the last day of each month at 09:00 via Etienne Zink's link.",
    )
  })

  it('omits the owner when the link user is not listed', () => {
    expect(autoReminderNote({ payment_link_user_id: 'unknown', hour: 10 }, links)).toBe(
      'Sent automatically on the last day of each month at 10:00.',
    )
  })
})

describe('buildPaymentUrl', () => {
  it('appends a two-decimal euro amount to the payment path', () => {
    expect(buildPaymentUrl('https://paypal.me/MoritzFl/', 1220)).toBe(
      'https://paypal.me/MoritzFl/12.20',
    )
  })

  it('preserves query parameters on other payment link formats', () => {
    expect(buildPaymentUrl('https://www.paypal.com/pay?receiver=abc', 50)).toBe(
      'https://www.paypal.com/pay/0.50?receiver=abc',
    )
  })
})

function paymentEntry(
  userId: string,
  displayName: string,
  amountDueCents: number,
  emailAvailable: boolean,
): PaymentSummary {
  return {
    user_id: userId,
    display_name: displayName,
    user_type: 'member',
    unpaid_amount_cents: amountDueCents,
    unpaid_drink_count: amountDueCents > 0 ? 1 : 0,
    paid_amount_cents: 0,
    paid_drink_count: 0,
    expense_amount_cents: 0,
    received_payment_amount_cents: 0,
    expense_balance_cents: 0,
    last_paid_at: null,
    email_available: emailAvailable,
  }
}

const reminderEntries = [
  paymentEntry('target', 'Target Person', 100, true),
  paymentEntry('guest', 'Guest Person', 50, false),
  paymentEntry('settled', 'Settled Person', 0, true),
]

function EmailDialogHarness({ onConfirm }: { onConfirm: () => void }) {
  const [mode, setMode] = useState<'all' | 'selected'>('selected')
  const [selectedUserIds, setSelectedUserIds] = useState<string[]>([])
  return createElement(EmailReminderDialog, {
    entries: reminderEntries,
    mode,
    selectedUserIds,
    isPending: false,
    onModeChange: setMode,
    onSelectionChange: setSelectedUserIds,
    onCancel: () => undefined,
    onConfirm,
  })
}

describe('EmailReminderDialog', () => {
  it('allows a custom subset and disables debtors without email', async () => {
    const user = userEvent.setup()
    const onConfirm = vi.fn()
    render(createElement(EmailDialogHarness, { onConfirm }))

    const target = screen.getByRole('checkbox', { name: /Target Person/ })
    const guest = screen.getByRole('checkbox', { name: /Guest Person/ })
    expect(target).toBeEnabled()
    expect(guest).toBeDisabled()
    expect(screen.queryByText('Settled Person')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Send 0 emails' })).toBeDisabled()

    await user.click(target)
    await user.click(screen.getByRole('button', { name: 'Send 1 email' }))
    expect(onConfirm).toHaveBeenCalledOnce()
  })

  it('selects every deliverable debtor without selecting missing-email users', async () => {
    const user = userEvent.setup()
    const onConfirm = vi.fn()
    render(createElement(EmailDialogHarness, { onConfirm }))

    await user.click(screen.getByRole('radio', { name: /Everyone/ }))
    expect(screen.getByRole('checkbox', { name: /Target Person/ })).toBeChecked()
    expect(screen.getByRole('checkbox', { name: /Guest Person/ })).not.toBeChecked()
    await user.click(screen.getByRole('button', { name: 'Send 1 email' }))
    expect(onConfirm).toHaveBeenCalledOnce()
  })
})
