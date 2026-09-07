import { describe, expect, it } from 'vitest'
import { debtTotals, formatEuros, paymentAmountDueCents } from './money'

describe('money helpers', () => {
  it('formats cent amounts as euros', () => {
    expect(formatEuros(1234).replace(/\s/g, ' ')).toBe('12,34 €')
    expect(formatEuros(-1234).replace(/\s/g, ' ')).toBe('-12,34 €')
  })

  it('derives current, paid, total, and signed balance', () => {
    expect(
      debtTotals({
        unpaid_amount_cents: 250,
        paid_amount_cents: 750,
      }),
    ).toEqual({
      currentDebtCents: 250,
      paidDebtCents: 750,
      totalDebtCents: 1000,
      expenseAmountCents: 0,
      receivedPaymentCents: 0,
      expenseBalanceCents: 0,
      balanceCents: -250,
      amountDueCents: 250,
    })
    expect(
      debtTotals({
        unpaid_amount_cents: 0,
        paid_amount_cents: 100,
      }).balanceCents,
    ).toBe(0)
    expect(
      debtTotals({
        unpaid_amount_cents: 500,
        paid_amount_cents: 100,
        expense_amount_cents: 4500,
        received_payment_amount_cents: 300,
        expense_balance_cents: 4200,
      }).balanceCents,
    ).toBe(3700)
  })

  it('derives the cash amount due from debt and expense balance', () => {
    expect(
      paymentAmountDueCents({
        unpaid_amount_cents: 1300,
        expense_balance_cents: 800,
      }),
    ).toBe(500)
    expect(
      paymentAmountDueCents({
        unpaid_amount_cents: 1300,
        expense_balance_cents: 1500,
      }),
    ).toBe(0)
    expect(
      paymentAmountDueCents({
        unpaid_amount_cents: 1300,
        expense_balance_cents: -200,
      }),
    ).toBe(1500)
  })
})
