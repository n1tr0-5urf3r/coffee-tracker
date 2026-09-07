export function formatEuros(cents: number | null | undefined) {
  return new Intl.NumberFormat('de-DE', {
    style: 'currency',
    currency: 'EUR',
  }).format((cents ?? 0) / 100)
}

export function debtTotals(payment: {
  unpaid_amount_cents: number
  paid_amount_cents: number
  expense_amount_cents?: number
  received_payment_amount_cents?: number
  expense_balance_cents?: number
}) {
  const expenseAmountCents = payment.expense_amount_cents ?? 0
  const receivedPaymentCents = payment.received_payment_amount_cents ?? 0
  const expenseBalanceCents = payment.expense_balance_cents ?? 0
  const balanceCents = expenseBalanceCents - payment.unpaid_amount_cents
  return {
    currentDebtCents: payment.unpaid_amount_cents,
    paidDebtCents: payment.paid_amount_cents,
    totalDebtCents: payment.unpaid_amount_cents + payment.paid_amount_cents,
    expenseAmountCents,
    receivedPaymentCents,
    expenseBalanceCents,
    balanceCents,
    amountDueCents: amountDueFromBalanceCents(balanceCents),
  }
}

export function amountDueFromBalanceCents(balanceCents: number) {
  return Math.max(-balanceCents, 0)
}

export function paymentAmountDueCents(payment: {
  unpaid_amount_cents: number
  expense_balance_cents?: number
}) {
  return amountDueFromBalanceCents(
    (payment.expense_balance_cents ?? 0) - payment.unpaid_amount_cents,
  )
}
