import { describe, expect, it } from 'vitest'
import { debtTotals, formatEuros, paymentAmountDueCents, paymentCashCents } from './money'

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

describe('paymentCashCents', () => {
  it('adds the settled expense balance to the tab', () => {
    expect(paymentCashCents({ amount_cents: 3325, expense_settled_cents: 2492 })).toBe(5817)
    expect(paymentCashCents({ amount_cents: 50, expense_settled_cents: -50 })).toBe(0)
    expect(paymentCashCents({ amount_cents: 100 })).toBe(100)
  })
})

describe('paymentAmountDueCents', () => {
  it("prefers the backend's amount due", () => {
    const kasse = { unpaid_amount_cents: 2375, expense_balance_cents: -9654 }
    expect(paymentAmountDueCents(kasse)).toBe(12029)
    expect(paymentAmountDueCents({ ...kasse, amount_due_cents: 2375 })).toBe(2375)
  })
})
