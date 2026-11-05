import { useState } from 'react'
import { useMarkPaidMutation, useMe, useMyPayment, usePaymentRecipients, usePayments, useReviewPaymentMutation } from '../api/queries'
import { Button } from '../components/Button'
import { Card, Skeleton } from '../components/Card'
import { branding } from '../config/branding'
import { formatEuros, paymentAmountDueCents, paymentCashCents } from '../lib/money'
import { buildPaymentUrl } from '../lib/paymentLinks'

export function SimplePaymentsPage() {
  const { data: me } = useMe()
  const isStaff = Boolean(me?.user.stock_manager)
  const incomingPayments = usePayments(isStaff)
  const reviewPayment = useReviewPaymentMutation()
  const payment = useMyPayment()
  const recipients = usePaymentRecipients()
  const markPaid = useMarkPaidMutation()
  const [confirming, setConfirming] = useState(false)
  const [recipientId, setRecipientId] = useState('')
  const [copyStatus, setCopyStatus] = useState<'copied' | 'error' | null>(null)
  const amountDue = payment.data ? paymentAmountDueCents(payment.data) : 0
  const selectedRecipient = recipients.data?.entries.find((recipient) => recipient.user_id === recipientId)
  const paymentUrl = selectedRecipient?.paypal_link && amountDue > 0
    ? buildPaymentUrl(selectedRecipient.paypal_link, amountDue)
    : null
  const pendingPayment = payment.data?.pending_payment
  const canSettle = Boolean(payment.data && (payment.data.unpaid_drink_count > 0 || payment.data.unpaid_amount_cents > 0 || amountDue > 0)) && !pendingPayment
  const pendingReceived = incomingPayments.data?.received_payments.filter((entry) => entry.status === 'pending') ?? []

  const confirmPayment = () => {
    if (!payment.data || !selectedRecipient) return
    markPaid.mutate(
      { user_id: payment.data.user_id, received_by_user_id: selectedRecipient.user_id, note: `Marked paid from ${branding.organizationShortName} payments` },
      { onSuccess: () => { setConfirming(false); setRecipientId(''); setCopyStatus(null) } },
    )
  }

  return (
    <div>
      <h1 className="mb-6 font-display text-2xl font-black tracking-tight sm:text-3xl">Payments</h1>
      {isStaff && incomingPayments.isError && (
        <Card className="mb-4" role="alert">
          <p>Payments awaiting your confirmation could not be loaded.</p>
          <button type="button" className="mt-2 font-bold text-primary underline dark:text-blush" onClick={() => void incomingPayments.refetch()}>Try again</button>
        </Card>
      )}
      {pendingReceived.length > 0 && (
        <Card className="mb-4">
          <h2 className="font-display text-lg font-black">Waiting for your confirmation</h2>
          <p className="mt-1 text-sm">Confirm after the money arrives, or reject to reopen the tab.</p>
          <ul className="mt-4 space-y-4">
            {pendingReceived.map((entry) => (
              <li key={entry.id} className="rounded-2xl bg-berry-100 p-4 dark:bg-white/10">
                <p className="font-bold">{entry.payer_display_name} · {formatEuros(paymentCashCents(entry))}</p>
                <div className="mt-3 flex flex-wrap gap-2">
                  <Button type="button" disabled={reviewPayment.isPending} onClick={() => reviewPayment.mutate({ id: entry.id, accept: true })}>Confirm</Button>
                  <Button type="button" variant="secondary" disabled={reviewPayment.isPending} onClick={() => reviewPayment.mutate({ id: entry.id, accept: false })}>Reject</Button>
                </div>
              </li>
            ))}
          </ul>
          {reviewPayment.isError && <p role="alert" className="mt-3 text-sm">Payment could not be updated. Please try again.</p>}
        </Card>
      )}
      {payment.isLoading ? (
        <Skeleton className="h-32" />
      ) : payment.isError || !payment.data ? (
        <Card role="alert">
          <p>Your amount due could not be loaded.</p>
          <button type="button" className="mt-2 font-bold text-primary underline dark:text-blush" onClick={() => void payment.refetch()}>Try again</button>
        </Card>
      ) : (
        <Card>
          <p className="text-sm font-bold text-berry-700 dark:text-rose-100">Your amount due</p>
          <p className="mt-2 font-display text-4xl font-black tabular-nums text-primary dark:text-blush">
            {formatEuros(amountDue)}
          </p>

          {pendingPayment && (
            <p role="status" className="mt-5 rounded-2xl bg-berry-100 p-4 text-sm dark:bg-white/10">
              Your payment of {formatEuros(paymentCashCents(pendingPayment))} is waiting for{' '}
              {recipients.data?.entries.find((recipient) => recipient.user_id === pendingPayment.received_by_user_id)?.display_name ?? 'the recipient'} to confirm it arrived.
            </p>
          )}
          {canSettle && (
            <div className="mt-6 border-t border-primary/10 pt-5">
              {recipients.isLoading ? <p className="text-sm">Loading recipients…</p> : recipients.isError ? (
                <p role="alert" className="text-sm">Recipients could not be loaded.{' '}
                  <button type="button" className="font-bold text-primary underline dark:text-blush" onClick={() => void recipients.refetch()}>Try again</button>
                </p>
              ) : recipients.data?.entries.length ? (
                <>
                  <label className="block text-sm font-bold">
                    Pay to
                    <select value={recipientId} disabled={markPaid.isPending} onChange={(event) => { setRecipientId(event.target.value); setCopyStatus(null); setConfirming(false); markPaid.reset() }} className="mt-2 min-h-12 w-full rounded-2xl border-2 border-primary/10 bg-card px-3 dark:bg-berry-950">
                      <option value="">Select a staff member</option>
                      {recipients.data.entries.map((recipient) => <option key={recipient.user_id} value={recipient.user_id}>{recipient.display_name}</option>)}
                    </select>
                  </label>
                  {selectedRecipient && (
                    <>
                      {paymentUrl ? (
                        <div className="mt-3 flex flex-wrap gap-2">
                          <a href={paymentUrl} target="_blank" rel="noreferrer" className="inline-flex min-h-12 items-center justify-center rounded-3xl bg-primary px-5 py-3 font-display text-sm font-black text-white shadow-button dark:bg-blush dark:text-berry-950">
                            Pay online
                          </a>
                          <Button type="button" variant="secondary" onClick={async () => {
                            try {
                              await navigator.clipboard.writeText(paymentUrl)
                              setCopyStatus('copied')
                            } catch {
                              setCopyStatus('error')
                            }
                          }}>Copy payment link</Button>
                        </div>
                      ) : amountDue > 0 ? (
                        <p className="mt-3 text-sm">No online payment link for {selectedRecipient.display_name}. You can pay them directly.</p>
                      ) : null}
                      {copyStatus === 'copied' && <p role="status" className="mt-2 text-sm">Payment link copied.</p>}
                      {copyStatus === 'error' && <p role="alert" className="mt-2 text-sm">Could not copy the link. You can copy it from Pay online.</p>}
                      <div className="mt-5 border-t border-primary/10 pt-5">
                        {!confirming ? (
                          <Button type="button" variant="secondary" onClick={() => { markPaid.reset(); setConfirming(true) }}>Mark paid</Button>
                        ) : (
                          <div>
                            <p className="font-display font-black">Confirm payment to {selectedRecipient.display_name}</p>
                            <p className="mt-1 text-sm text-berry-700 dark:text-rose-100">Submit your tab after any payment is made. The amount due is {formatEuros(amountDue)}. If you paid another staff member, they must confirm it arrived.</p>
                            {markPaid.isError && <p role="alert" className="mt-3 text-sm text-primary dark:text-blush">Payment could not be recorded. Please try again.</p>}
                            <div className="mt-4 flex flex-wrap gap-2">
                              <Button type="button" disabled={markPaid.isPending} onClick={confirmPayment}>Confirm payment</Button>
                              <Button type="button" variant="ghost" disabled={markPaid.isPending} onClick={() => setConfirming(false)}>Cancel</Button>
                            </div>
                          </div>
                        )}
                      </div>
                    </>
                  )}
                </>
              ) : <p className="text-sm">No staff recipients are configured.</p>}
            </div>
          )}
          {markPaid.isSuccess && markPaid.data?.status === 'confirmed' && <p role="status" className="mt-4 text-sm font-bold">Payment confirmed.</p>}
          {markPaid.isSuccess && markPaid.data?.status === 'pending' && !pendingPayment && <p role="status" className="mt-4 text-sm font-bold">Payment submitted; awaiting recipient confirmation.</p>}
        </Card>
      )}
    </div>
  )
}
