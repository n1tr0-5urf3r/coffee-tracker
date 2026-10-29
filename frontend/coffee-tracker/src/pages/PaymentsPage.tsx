import { Banknote, CalendarClock, CheckCircle2, Copy, ExternalLink, Mail, Plus, ReceiptText, Send } from 'lucide-react'
import { useEffect, useMemo, useState, type FormEvent } from 'react'
import {
  useCreateExpenseMutation,
  useEmailPaymentReminderMutation,
  useExpenses,
  useMarkPaidMutation,
  useMe,
  useMyPayment,
  usePaymentReminderMutation,
  usePaymentConfig,
  usePaymentRecipients,
  usePayments,
  usePaymentsLinks,
} from '../api/queries'
import type {
  AutoReminderInfo,
  ExpenseEntry,
  ExpenseSummary,
  MyPaymentResponse,
  PaymentSummary,
  PaypalLinkEntry,
  PaymentRecipient,
  ReceivedPaymentEntry,
} from '../api/client'
import { Button } from '../components/Button'
import { Card, Skeleton } from '../components/Card'
import { PageHeader } from '../components/PageHeader'
import { formatUnixDate } from '../lib/date'
import { formatEuros, paymentAmountDueCents } from '../lib/money'
import { useToast } from '../lib/toast'
import { branding } from '../config/branding'

export function PaymentsPage() {
  const { data: me } = useMe()
  const isStaff = Boolean(me?.user.stock_manager)
  const payments = usePayments(isStaff)
  const myPayment = useMyPayment()
  const paymentLinks = usePaymentsLinks(true)
  const paymentConfig = usePaymentConfig(isStaff)
  const paymentRecipients = usePaymentRecipients(true)
  const expenses = useExpenses(isStaff)
  const createExpense = useCreateExpenseMutation()
  const markPaid = useMarkPaidMutation()
  const paymentReminder = usePaymentReminderMutation()
  const emailPaymentReminder = useEmailPaymentReminderMutation()
  const { pushToast } = useToast()
  const [markPaidDraft, setMarkPaidDraft] = useState<{
    entry: PaymentSummary
    recipientId: string
  } | null>(null)
  const [emailReminderDraft, setEmailReminderDraft] = useState<{
    paymentLinkUserId: string
    mode: 'all' | 'selected'
    selectedUserIds: string[]
  } | null>(null)

  const sendPaymentReminder = (paymentLinkUserId: string) => {
    paymentReminder.mutate(
      { payment_link_user_id: paymentLinkUserId },
      {
        onSuccess: (response) => {
          pushToast({
            tone: 'success',
            title: 'Payment reminder sent',
            description: `${response.reminded_count} open tab${response.reminded_count === 1 ? '' : 's'} included.`,
          })
        },
        onError: (error) => {
          pushToast({
            tone: 'error',
            title: 'Reminder not sent',
            description:
              error instanceof Error
                ? error.message
                : 'The reminder could not be sent.',
          })
        },
      },
    )
  }

  const sendEmailPaymentReminder = () => {
    if (!emailReminderDraft) return
    emailPaymentReminder.mutate(
      {
        payment_link_user_id: emailReminderDraft.paymentLinkUserId,
        recipient_user_ids:
          emailReminderDraft.mode === 'all'
            ? null
            : emailReminderDraft.selectedUserIds,
      },
      {
        onSuccess: (response) => {
          if (response.failed_user_ids.length) {
            setEmailReminderDraft((draft) =>
              draft
                ? {
                    ...draft,
                    mode: 'selected',
                    selectedUserIds: response.failed_user_ids,
                  }
                : draft,
            )
            pushToast({
              tone: response.sent_count ? 'info' : 'error',
              title: response.sent_count
                ? 'Some reminders were not sent'
                : 'Email reminders not sent',
              description: `${response.sent_count} sent, ${response.failed_user_ids.length} failed.${
                response.skipped_no_email_count
                  ? ` ${response.skipped_no_email_count} skipped without email.`
                  : ''
              } Only failed recipients remain selected.`,
            })
            return
          }

          setEmailReminderDraft(null)
          pushToast({
            tone: 'success',
            title: 'Email reminders sent',
            description: `${response.sent_count} email${response.sent_count === 1 ? '' : 's'} accepted by the SMTP server.${
              response.skipped_no_email_count
                ? ` ${response.skipped_no_email_count} skipped without email.`
                : ''
            }`,
          })
        },
        onError: (error) => {
          pushToast({
            tone: 'error',
            title: 'Email reminders not sent',
            description:
              error instanceof Error
                ? error.message
                : 'The email reminders could not be sent.',
          })
        },
      },
    )
  }

  const entries: PaymentSummary[] = isStaff
    ? payments.data?.entries ?? []
    : myPayment.data
      ? [
          {
            user_id: myPayment.data.user_id,
            display_name: me?.user.display_name ?? 'You',
            user_type: me?.user.user_type ?? 'guest',
            unpaid_amount_cents: myPayment.data.unpaid_amount_cents,
            unpaid_drink_count: myPayment.data.unpaid_drink_count,
            paid_amount_cents: myPayment.data.paid_amount_cents,
            paid_drink_count: myPayment.data.paid_drink_count,
            expense_amount_cents: myPayment.data.expense_amount_cents,
            received_payment_amount_cents: myPayment.data.received_payment_amount_cents,
            expense_balance_cents: myPayment.data.expense_balance_cents,
            last_paid_at: myPayment.data.last_paid_at,
            email_available: false,
          },
        ]
      : []
  const displayedEntries = me?.user.id
    ? [...entries].sort((first, second) => {
        if (first.user_id === me.user.id) return -1
        if (second.user_id === me.user.id) return 1
        return 0
      })
    : entries

  const isLoading = isStaff ? payments.isLoading || myPayment.isLoading : myPayment.isLoading
  const recipients = paymentRecipients.data?.entries ?? []
  const openMarkPaidDialog = (entry: PaymentSummary) => {
    setMarkPaidDraft({ entry, recipientId: '' })
  }
  const confirmMarkPaid = () => {
    if (!markPaidDraft?.recipientId) return
    markPaid.mutate(
      {
        user_id: markPaidDraft.entry.user_id,
        received_by_user_id: markPaidDraft.recipientId,
        note: `Marked paid from ${branding.organizationShortName} payments`,
      },
      {
        onSuccess: () => setMarkPaidDraft(null),
        onError: (error) => {
          pushToast({
            tone: 'error',
            title: 'Payment not marked',
            description:
              error instanceof Error
                ? error.message
                : 'The payment could not be marked as paid.',
          })
        },
      },
    )
  }

  return (
    <div>
      <PageHeader eyebrow={isStaff ? `${branding.organizationShortName} staff tools` : 'Payments'} title="Payments" />

      <PaymentLinkGenerator
        links={paymentLinks.data?.entries ?? []}
        linksLoading={paymentLinks.isLoading}
        payment={myPayment.data}
        canSendReminders={isStaff}
        reminderPending={paymentReminder.isPending}
        emailReminderPending={emailPaymentReminder.isPending}
        emailRemindersConfigured={Boolean(paymentConfig.data?.email_reminders_configured)}
        emailConfigLoading={paymentConfig.isLoading}
        autoReminder={paymentConfig.data?.auto_reminder ?? null}
        onSendReminder={sendPaymentReminder}
        onOpenEmailReminder={(paymentLinkUserId) =>
          setEmailReminderDraft({
            paymentLinkUserId,
            mode: 'selected',
            selectedUserIds: [],
          })
        }
      />

      {isLoading ? (
        <Skeleton className="h-72" />
      ) : (
        <Card>
          <div className="mb-5 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div>
              <div className="mb-2 flex items-center gap-3">
                <Banknote className="size-6 text-primary dark:text-blush" />
                <p className="font-display text-xl font-black">Payment tracking</p>
              </div>
              <p className="text-sm font-bold text-berry-700 dark:text-rose-100">
                {isStaff
                  ? 'Current unpaid tabs and settled usage for everyone.'
                  : 'Your own current and already settled usage.'}
              </p>
            </div>
          </div>

          {displayedEntries.length ? (
            <>
              <div className="space-y-3 md:hidden">
                {displayedEntries.map((entry) => (
                  <PaymentEntryCard
                    key={entry.user_id}
                    entry={entry}
                    isCurrentUser={entry.user_id === me?.user.id}
                    canMarkEntry={Boolean(me?.user.stock_manager) || entry.user_id === me?.user.id}
                    isPending={markPaid.isPending || paymentRecipients.isLoading}
                    hasRecipients={recipients.length > 0}
                    onMarkPaid={() => openMarkPaidDialog(entry)}
                  />
                ))}
              </div>

              <div className="hidden overflow-x-auto md:block">
                <table className="w-full min-w-[46rem] border-separate border-spacing-y-2 text-left">
                  <thead>
                    <tr className="text-xs font-black uppercase tracking-[0.14em] text-berry-700 dark:text-rose-100">
                      <th className="px-3 py-2">User</th>
                      <th className="px-3 py-2">Current debt</th>
                      <th className="px-3 py-2">Expenses</th>
                      <th className="px-3 py-2">Amount due</th>
                      <th className="px-3 py-2">Unpaid cups</th>
                      <th className="px-3 py-2">Already paid</th>
                      <th className="px-3 py-2">Paid cups</th>
                      <th className="px-3 py-2">Last paid</th>
                      <th className="px-3 py-2 text-right">Action</th>
                    </tr>
                  </thead>
                  <tbody>
                    {displayedEntries.map((entry) => {
                      const canMarkEntry =
                        Boolean(me?.user.stock_manager) || entry.user_id === me?.user.id
                      const isCurrentUser = entry.user_id === me?.user.id
                      const amountDueCents = paymentAmountDueCents(entry)
                      return (
                        <tr
                          key={entry.user_id}
                          className={
                            isCurrentUser
                              ? 'bg-primary/15 font-bold text-berry-950 ring-2 ring-primary/40 dark:bg-blush/15 dark:text-berry-50 dark:ring-blush/40'
                              : 'bg-berry-100 font-bold text-berry-950 dark:bg-white/10 dark:text-berry-50'
                          }
                        >
                          <td className="rounded-l-2xl px-3 py-3">
                            <span className="flex flex-wrap items-center gap-2 font-display font-black">
                              {entry.display_name}
                              {isCurrentUser ? (
                                <span className="rounded-full bg-primary px-2 py-0.5 text-[0.65rem] font-black uppercase tracking-[0.12em] text-white dark:bg-blush dark:text-berry-950">
                                  You
                                </span>
                              ) : null}
                            </span>
                            <span className="text-xs uppercase text-berry-700 dark:text-rose-100">
                              {entry.user_type}
                            </span>
                          </td>
                          <td className="px-3 py-3">{formatEuros(entry.unpaid_amount_cents)}</td>
                          <td className="px-3 py-3">
                            {formatEuros(entry.expense_amount_cents)}
                          </td>
                          <td className="px-3 py-3">
                            {formatEuros(amountDueCents)}
                          </td>
                          <td className="px-3 py-3">{entry.unpaid_drink_count}</td>
                          <td className="px-3 py-3">{formatEuros(entry.paid_amount_cents)}</td>
                          <td className="px-3 py-3">{entry.paid_drink_count}</td>
                          <td className="px-3 py-3">
                            {formatUnixDate(entry.last_paid_at, 'Never')}
                          </td>
                          <td className="rounded-r-2xl px-3 py-3 text-right">
                            {canMarkEntry && entry.unpaid_drink_count > 0 ? (
                              <Button
                                className="whitespace-nowrap"
                                disabled={
                                  markPaid.isPending ||
                                  paymentRecipients.isLoading ||
                                  recipients.length === 0
                                }
                                onClick={() => openMarkPaidDialog(entry)}
                              >
                                <CheckCircle2 className="size-4" />
                                Mark paid
                              </Button>
                            ) : canMarkEntry ? (
                              <span className="text-sm font-black text-berry-700 dark:text-rose-100">
                                Settled
                              </span>
                            ) : (
                              <span className="text-sm font-black text-berry-700 dark:text-rose-100">
                                -
                              </span>
                            )}
                          </td>
                        </tr>
                      )
                    })}
                  </tbody>
                </table>
              </div>
            </>
          ) : (
            <p className="font-bold text-berry-700 dark:text-rose-100">No payment entries yet.</p>
          )}
        </Card>
      )}

      {markPaidDraft ? (
        <MarkPaidDialog
          entry={markPaidDraft.entry}
          recipients={recipients}
          selectedRecipientId={markPaidDraft.recipientId}
          isPending={markPaid.isPending}
          onRecipientChange={(recipientId) =>
            setMarkPaidDraft((draft) => (draft ? { ...draft, recipientId } : draft))
          }
          onCancel={() => setMarkPaidDraft(null)}
          onConfirm={confirmMarkPaid}
        />
      ) : null}

      {emailReminderDraft ? (
        <EmailReminderDialog
          entries={payments.data?.entries ?? []}
          mode={emailReminderDraft.mode}
          selectedUserIds={emailReminderDraft.selectedUserIds}
          isPending={emailPaymentReminder.isPending}
          onModeChange={(mode) =>
            setEmailReminderDraft((draft) => (draft ? { ...draft, mode } : draft))
          }
          onSelectionChange={(selectedUserIds) =>
            setEmailReminderDraft((draft) =>
              draft ? { ...draft, selectedUserIds } : draft,
            )
          }
          onCancel={() => setEmailReminderDraft(null)}
          onConfirm={sendEmailPaymentReminder}
        />
      ) : null}

      {isStaff ? (
        <ReceivedPaymentsOverview
          entries={payments.data?.received_payments ?? []}
          loading={payments.isLoading}
        />
      ) : null}

      {isStaff ? (
        <ExpensesTracker
          entries={expenses.data?.entries ?? []}
          summaries={expenses.data?.summaries ?? []}
          loading={expenses.isLoading}
          createPending={createExpense.isPending}
          onCreate={(description, amountCents) =>
            createExpense.mutate(
              { description, amount_cents: amountCents },
              {
                onSuccess: () => {
                  pushToast({
                    tone: 'success',
                    title: 'Expense logged',
                    description: 'The shared balance was updated.',
                  })
                },
                onError: (error) => {
                  pushToast({
                    tone: 'error',
                    title: 'Expense not logged',
                    description:
                      error instanceof Error
                        ? error.message
                        : 'The expense could not be logged.',
                  })
                },
              },
            )
          }
        />
      ) : null}
    </div>
  )
}

function PaymentLinkGenerator({
  links,
  linksLoading,
  payment,
  canSendReminders,
  reminderPending,
  emailReminderPending,
  emailRemindersConfigured,
  emailConfigLoading,
  autoReminder,
  onSendReminder,
  onOpenEmailReminder,
}: {
  links: PaypalLinkEntry[]
  linksLoading: boolean
  payment?: MyPaymentResponse
  canSendReminders: boolean
  reminderPending: boolean
  emailReminderPending: boolean
  emailRemindersConfigured: boolean
  emailConfigLoading: boolean
  autoReminder: AutoReminderInfo | null
  onSendReminder: (paymentLinkUserId: string) => void
  onOpenEmailReminder: (paymentLinkUserId: string) => void
}) {
  const [selectedUserId, setSelectedUserId] = useState('')
  const [copied, setCopied] = useState(false)

  useEffect(() => {
    if (!selectedUserId && links.length) {
      setSelectedUserId(links[0].user_id)
    }
  }, [links, selectedUserId])

  const selectedLink = useMemo(
    () => links.find((link) => link.user_id === selectedUserId) ?? links[0],
    [links, selectedUserId],
  )
  const unpaidAmountCents = payment?.unpaid_amount_cents ?? 0
  const expenseBalanceCents = payment?.expense_balance_cents ?? 0
  const amountDueCents = payment ? paymentAmountDueCents(payment) : 0
  const generatedUrl =
    selectedLink && amountDueCents > 0
      ? buildPaymentUrl(selectedLink.paypal_link, amountDueCents)
      : null

  const copyPaymentUrl = async () => {
    if (!generatedUrl) return
    await navigator.clipboard.writeText(generatedUrl)
    setCopied(true)
    window.setTimeout(() => setCopied(false), 2000)
  }

  return (
    <Card className="mx-auto mb-5 max-w-3xl border-primary/20 bg-[#fff8e8] dark:bg-berry-900">
      <div className="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
        <div>
          <div className="mb-2 flex items-center gap-3">
            <Banknote className="size-6 text-primary dark:text-blush" />
            <p className="font-display text-xl font-black">Generate payment link</p>
          </div>
          <p className="text-sm font-bold text-berry-700 dark:text-rose-100">
            Your amount due is{' '}
            <span className="font-black text-primary dark:text-blush">
              {formatEuros(amountDueCents)}
            </span>
            .
          </p>
          {payment ? (
            <p className="mt-1 text-xs font-black uppercase tracking-[0.12em] text-berry-700/80 dark:text-rose-100/80">
              Debt {formatEuros(unpaidAmountCents)} · Expense balance{' '}
              {formatEuros(expenseBalanceCents)}
            </p>
          ) : null}
        </div>

        <div className="min-w-0 md:w-72">
          <label className="mb-2 block text-xs font-black uppercase tracking-[0.14em] text-berry-700 dark:text-rose-100">
            Pay to
          </label>
          <select
            className="min-h-12 w-full rounded-2xl border-2 border-primary/10 bg-[#fffdf8] px-3 font-bold text-berry-950 outline-none transition focus:border-primary dark:bg-berry-950 dark:text-rose-50"
            disabled={linksLoading || !links.length}
            value={selectedLink?.user_id ?? ''}
            onChange={(event) => setSelectedUserId(event.target.value)}
          >
            {links.length ? (
              links.map((link) => (
                <option key={link.user_id} value={link.user_id}>
                  {link.display_name}
                </option>
              ))
            ) : (
              <option value="">No staff links configured</option>
            )}
          </select>
          {canSendReminders ? (
            <div className="mt-3 grid grid-cols-2 gap-2">
              <Button
                type="button"
                className="w-full px-3"
                disabled={reminderPending || linksLoading || !selectedLink}
                onClick={() => {
                  if (selectedLink) {
                    onSendReminder(selectedLink.user_id)
                  }
                }}
              >
                <Send className="size-4" />
                {reminderPending ? 'Sending' : 'Send Payment Reminder'}
              </Button>
              <Button
                type="button"
                variant="secondary"
                className="w-full px-3"
                disabled={
                  emailReminderPending ||
                  emailConfigLoading ||
                  !emailRemindersConfigured ||
                  linksLoading ||
                  !selectedLink
                }
                onClick={() => {
                  if (selectedLink) {
                    onOpenEmailReminder(selectedLink.user_id)
                  }
                }}
              >
                <Mail className="size-4" />
                Send Email Reminder
              </Button>
              {!emailConfigLoading && !emailRemindersConfigured ? (
                <p className="col-span-2 text-center text-xs font-bold text-berry-700 dark:text-rose-100">
                  SMTP is not configured.
                </p>
              ) : null}
              {autoReminder ? (
                <p className="col-span-2 flex items-center justify-center gap-1.5 text-center text-xs font-bold text-berry-700 dark:text-rose-100">
                  <CalendarClock className="size-3.5 shrink-0" />
                  {autoReminderNote(autoReminder, links)}
                </p>
              ) : null}
            </div>
          ) : null}
        </div>
      </div>

      {generatedUrl ? (
        <div className="mt-4 flex flex-col gap-2 rounded-3xl bg-white/70 p-3 dark:bg-white/10 sm:flex-row sm:items-center">
          <a
            href={generatedUrl}
            target="_blank"
            rel="noreferrer"
            className="inline-flex min-w-0 flex-1 items-center gap-2 break-all font-bold text-primary underline decoration-primary/30 underline-offset-4 dark:text-blush"
          >
            <ExternalLink className="size-4 shrink-0" />
            {generatedUrl}
          </a>
          <Button type="button" className="shrink-0" onClick={copyPaymentUrl}>
            <Copy className="size-4" />
            {copied ? 'Copied' : 'Copy'}
          </Button>
        </div>
      ) : (
        <p className="mt-4 rounded-3xl bg-white/70 p-3 text-sm font-black text-berry-700 dark:bg-white/10 dark:text-rose-100">
          {amountDueCents <= 0
            ? 'No payment due right now.'
            : 'No staff payment links are configured yet.'}
        </p>
      )}
    </Card>
  )
}

export function autoReminderNote(autoReminder: AutoReminderInfo, links: PaypalLinkEntry[]) {
  const time = `${String(autoReminder.hour).padStart(2, '0')}:00`
  const linkOwner = links.find((link) => link.user_id === autoReminder.payment_link_user_id)
  const via = linkOwner ? ` via ${linkOwner.display_name}'s link` : ''
  return `Sent automatically on the last day of each month at ${time}${via}.`
}

export function buildPaymentUrl(baseLink: string, amountCents: number) {
  const amount = (amountCents / 100).toFixed(2)
  const trimmed = baseLink.trim()
  try {
    const url = new URL(trimmed)
    url.pathname = `${url.pathname.replace(/\/+$/, '')}/${amount}`
    return url.toString()
  } catch {
    return `${trimmed.replace(/\/+$/, '')}/${amount}`
  }
}

function PaymentEntryCard({
  entry,
  isCurrentUser,
  canMarkEntry,
  isPending,
  hasRecipients,
  onMarkPaid,
}: {
  entry: PaymentSummary
  isCurrentUser: boolean
  canMarkEntry: boolean
  isPending: boolean
  hasRecipients: boolean
  onMarkPaid: () => void
}) {
  const amountDueCents = paymentAmountDueCents(entry)
  const balanceCents = entry.expense_balance_cents - entry.unpaid_amount_cents
  return (
    <div
      className={
        isCurrentUser
          ? 'rounded-3xl bg-primary/15 p-4 font-bold text-berry-950 ring-2 ring-primary/40 dark:bg-blush/15 dark:text-berry-50 dark:ring-blush/40'
          : 'rounded-3xl bg-berry-100 p-4 font-bold text-berry-950 dark:bg-white/10 dark:text-berry-50'
      }
    >
      <div className="mb-3 flex items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="flex flex-wrap items-center gap-2 break-words font-display text-lg font-black">
            {entry.display_name}
            {isCurrentUser ? (
              <span className="rounded-full bg-primary px-2 py-0.5 text-[0.65rem] font-black uppercase tracking-[0.12em] text-white dark:bg-blush dark:text-berry-950">
                You
              </span>
            ) : null}
          </p>
          <p className="text-xs font-black uppercase text-berry-700 dark:text-rose-100">
            {entry.user_type}
          </p>
        </div>
        <div className="shrink-0 text-right">
          <p className="text-[0.65rem] font-black uppercase tracking-[0.12em] text-berry-700 dark:text-rose-100">
            Amount due
          </p>
          <p className="font-display text-xl font-black text-primary dark:text-blush">
            {formatEuros(amountDueCents)}
          </p>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-2 text-sm">
        <PaymentMetric label="Current debt" value={formatEuros(entry.unpaid_amount_cents)} />
        <PaymentMetric label="Expenses" value={formatEuros(entry.expense_amount_cents)} />
        <PaymentMetric label="Balance" value={formatEuros(balanceCents)} />
        <PaymentMetric label="Unpaid cups" value={String(entry.unpaid_drink_count)} />
        <PaymentMetric label="Already paid" value={formatEuros(entry.paid_amount_cents)} />
        <PaymentMetric label="Paid cups" value={String(entry.paid_drink_count)} />
        <PaymentMetric
          label="Last paid"
          value={formatUnixDate(entry.last_paid_at, 'Never')}
        />
      </div>

      <div className="mt-3">
        {canMarkEntry && entry.unpaid_drink_count > 0 ? (
          <Button className="w-full" disabled={isPending || !hasRecipients} onClick={onMarkPaid}>
            <CheckCircle2 className="size-4" />
            Mark paid
          </Button>
        ) : canMarkEntry ? (
          <p className="text-sm font-black text-berry-700 dark:text-rose-100">Settled</p>
        ) : null}
      </div>
    </div>
  )
}

export function EmailReminderDialog({
  entries,
  mode,
  selectedUserIds,
  isPending,
  onModeChange,
  onSelectionChange,
  onCancel,
  onConfirm,
}: {
  entries: PaymentSummary[]
  mode: 'all' | 'selected'
  selectedUserIds: string[]
  isPending: boolean
  onModeChange: (mode: 'all' | 'selected') => void
  onSelectionChange: (userIds: string[]) => void
  onCancel: () => void
  onConfirm: () => void
}) {
  const debtors = entries.filter((entry) => paymentAmountDueCents(entry) > 0)
  const deliverableIds = new Set(
    debtors.filter((entry) => entry.email_available).map((entry) => entry.user_id),
  )
  const selectedIds = new Set(selectedUserIds)
  const attemptCount =
    mode === 'all'
      ? deliverableIds.size
      : selectedUserIds.filter((userId) => deliverableIds.has(userId)).length
  const missingEmailCount = debtors.length - deliverableIds.size

  const updateSelection = (userId: string, checked: boolean) => {
    const next = new Set(selectedIds)
    if (checked) next.add(userId)
    else next.delete(userId)
    onSelectionChange([...next])
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-berry-950/70 px-4 py-6 backdrop-blur-sm">
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="email-reminder-title"
        className="max-h-full w-full max-w-lg overflow-y-auto rounded-4xl border-2 border-white/80 bg-[#fffdf8] p-5 text-berry-950 shadow-soft dark:border-white/10 dark:bg-berry-900 dark:text-berry-50"
      >
        <div className="mb-5 flex items-start justify-between gap-4">
          <div>
            <p id="email-reminder-title" className="font-display text-xl font-black">
              Send email reminders
            </p>
            <p className="mt-1 text-sm font-bold text-berry-700 dark:text-rose-100">
              Each person receives only their own balance and payment link.
            </p>
          </div>
          <Button
            type="button"
            variant="ghost"
            className="min-h-10 px-3"
            disabled={isPending}
            onClick={onCancel}
          >
            Cancel
          </Button>
        </div>

        <fieldset className="grid gap-2 sm:grid-cols-2">
          <legend className="mb-2 text-xs font-black uppercase tracking-[0.14em] text-berry-700 dark:text-rose-100">
            Recipients
          </legend>
          <label className="flex cursor-pointer items-start gap-3 rounded-2xl bg-white/70 p-3 font-bold ring-2 ring-primary/10 dark:bg-berry-950/40">
            <input
              type="radio"
              name="email-reminder-mode"
              className="mt-1 accent-primary"
              checked={mode === 'selected'}
              disabled={isPending}
              onChange={() => onModeChange('selected')}
            />
            <span>
              Choose people
              <span className="block text-xs font-medium text-berry-700 dark:text-rose-100">
                Pick any custom subset.
              </span>
            </span>
          </label>
          <label className="flex cursor-pointer items-start gap-3 rounded-2xl bg-white/70 p-3 font-bold ring-2 ring-primary/10 dark:bg-berry-950/40">
            <input
              type="radio"
              name="email-reminder-mode"
              className="mt-1 accent-primary"
              checked={mode === 'all'}
              disabled={isPending || deliverableIds.size === 0}
              onChange={() => onModeChange('all')}
            />
            <span>
              Everyone
              <span className="block text-xs font-medium text-berry-700 dark:text-rose-100">
                {deliverableIds.size} deliverable debtor
                {deliverableIds.size === 1 ? '' : 's'}.
              </span>
            </span>
          </label>
        </fieldset>

        <div className="mt-4 space-y-2">
          {debtors.length ? (
            debtors.map((entry) => {
              const available = entry.email_available
              const checked = mode === 'all' ? available : selectedIds.has(entry.user_id)
              return (
                <label
                  key={entry.user_id}
                  className={`flex items-center justify-between gap-3 rounded-2xl p-3 font-bold ${
                    available
                      ? 'cursor-pointer bg-berry-100 dark:bg-white/10'
                      : 'cursor-not-allowed bg-berry-100/60 opacity-60 dark:bg-white/5'
                  }`}
                >
                  <span className="flex min-w-0 items-center gap-3">
                    <input
                      type="checkbox"
                      className="size-4 shrink-0 accent-primary"
                      checked={checked}
                      disabled={isPending || mode === 'all' || !available}
                      onChange={(event) =>
                        updateSelection(entry.user_id, event.target.checked)
                      }
                    />
                    <span className="min-w-0">
                      <span className="block break-words">{entry.display_name}</span>
                      {!available ? (
                        <span className="block text-xs text-primary dark:text-blush">
                          No email in database
                        </span>
                      ) : null}
                    </span>
                  </span>
                  <span className="shrink-0 font-display text-primary dark:text-blush">
                    {formatEuros(paymentAmountDueCents(entry))}
                  </span>
                </label>
              )
            })
          ) : (
            <p className="rounded-2xl bg-berry-100 p-3 text-sm font-bold dark:bg-white/10">
              Nobody currently has an open payment balance.
            </p>
          )}
        </div>

        {missingEmailCount ? (
          <p className="mt-3 text-xs font-bold text-berry-700 dark:text-rose-100">
            {missingEmailCount} debtor{missingEmailCount === 1 ? '' : 's'} cannot receive email.
          </p>
        ) : null}

        <div className="mt-5 flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
          <Button type="button" variant="secondary" disabled={isPending} onClick={onCancel}>
            Cancel
          </Button>
          <Button
            type="button"
            disabled={isPending || attemptCount === 0}
            onClick={onConfirm}
          >
            <Mail className="size-4" />
            {isPending
              ? 'Sending emails'
              : `Send ${attemptCount} email${attemptCount === 1 ? '' : 's'}`}
          </Button>
        </div>
      </div>
    </div>
  )
}

function MarkPaidDialog({
  entry,
  recipients,
  selectedRecipientId,
  isPending,
  onRecipientChange,
  onCancel,
  onConfirm,
}: {
  entry: PaymentSummary
  recipients: PaymentRecipient[]
  selectedRecipientId: string
  isPending: boolean
  onRecipientChange: (userId: string) => void
  onCancel: () => void
  onConfirm: () => void
}) {
  const amountDueCents = paymentAmountDueCents(entry)
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-berry-950/70 px-4 py-6 backdrop-blur-sm">
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="mark-paid-title"
        className="w-full max-w-md rounded-4xl border-2 border-white/80 bg-[#fffdf8] p-5 text-berry-950 shadow-soft dark:border-white/10 dark:bg-berry-900 dark:text-berry-50"
      >
        <div className="mb-5 flex items-start justify-between gap-4">
          <div>
            <p id="mark-paid-title" className="font-display text-xl font-black">
              Mark paid
            </p>
            <p className="mt-1 text-sm font-bold text-berry-700 dark:text-rose-100">
              {entry.display_name} settles {formatEuros(entry.unpaid_amount_cents)}.
              Amount due: {formatEuros(amountDueCents)}.
            </p>
          </div>
          <Button type="button" variant="ghost" className="min-h-10 px-3" onClick={onCancel}>
            Cancel
          </Button>
        </div>

        <label className="mb-2 block text-xs font-black uppercase tracking-[0.14em] text-berry-700 dark:text-rose-100">
          Paid to
        </label>
        <RecipientSelect
          recipients={recipients}
          selectedRecipientId={selectedRecipientId}
          onRecipientChange={onRecipientChange}
          disabled={isPending}
        />

        <div className="mt-5 flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
          <Button type="button" variant="secondary" disabled={isPending} onClick={onCancel}>
            Cancel
          </Button>
          <Button
            type="button"
            disabled={isPending || !selectedRecipientId}
            onClick={onConfirm}
          >
            <CheckCircle2 className="size-4" />
            Confirm payment to {recipientName(recipients, selectedRecipientId)}
          </Button>
        </div>
      </div>
    </div>
  )
}

function RecipientSelect({
  recipients,
  selectedRecipientId,
  onRecipientChange,
  disabled,
}: {
  recipients: PaymentRecipient[]
  selectedRecipientId: string
  onRecipientChange: (userId: string) => void
  disabled?: boolean
}) {
  return (
    <select
      className="min-h-12 w-full rounded-2xl border-2 border-primary/10 bg-[#fffdf8] px-3 font-bold text-berry-950 outline-none transition focus:border-primary dark:bg-berry-950 dark:text-rose-50"
      disabled={disabled || !recipients.length}
      value={selectedRecipientId}
      onChange={(event) => onRecipientChange(event.target.value)}
    >
      {recipients.length ? (
        <>
          <option value="">----</option>
          {recipients.map((recipient) => (
            <option key={recipient.user_id} value={recipient.user_id}>
              {recipient.display_name}
            </option>
          ))}
        </>
      ) : (
        <option value="">No staff recipients</option>
      )}
    </select>
  )
}

function recipientName(recipients: PaymentRecipient[], userId: string) {
  return recipients.find((recipient) => recipient.user_id === userId)?.display_name ?? 'staff'
}

function PaymentMetric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-2xl bg-white/70 p-3 dark:bg-berry-950/40">
      <p className="text-[0.65rem] font-black uppercase tracking-[0.12em] text-berry-700 dark:text-rose-100">
        {label}
      </p>
      <p className="mt-1 break-words font-display font-black">{value}</p>
    </div>
  )
}

function ReceivedPaymentsOverview({
  entries,
  loading,
}: {
  entries: ReceivedPaymentEntry[]
  loading: boolean
}) {
  return (
    <Card className="mt-5">
      <div className="mb-5 flex items-center gap-3">
        <Banknote className="size-6 text-primary dark:text-blush" />
        <p className="font-display text-xl font-black">Received payments</p>
      </div>

      {loading ? (
        <Skeleton className="h-40" />
      ) : entries.length ? (
        <>
          <div className="space-y-3 md:hidden">
            {entries.map((entry) => (
              <div
                key={entry.id}
                className="rounded-3xl bg-berry-100 p-4 font-bold text-berry-950 dark:bg-white/10 dark:text-berry-50"
              >
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <p className="break-words font-display text-lg font-black">
                      {entry.payer_display_name}
                    </p>
                    <p className="text-xs font-black uppercase text-berry-700 dark:text-rose-100">
                      to {entry.recipient_display_name} · {formatUnixDate(entry.paid_at, 'Unknown')}
                    </p>
                  </div>
                  <p className="shrink-0 font-display text-xl font-black text-primary dark:text-blush">
                    {formatEuros(entry.amount_cents)}
                  </p>
                </div>
                <p className="mt-2 text-sm font-black text-berry-700 dark:text-rose-100">
                  {entry.drink_count} drink{entry.drink_count === 1 ? '' : 's'}
                </p>
              </div>
            ))}
          </div>

          <div className="hidden overflow-x-auto md:block">
            <table className="w-full min-w-[42rem] border-separate border-spacing-y-2 text-left">
              <thead>
                <tr className="text-xs font-black uppercase tracking-[0.14em] text-berry-700 dark:text-rose-100">
                  <th className="px-3 py-2">When</th>
                  <th className="px-3 py-2">From</th>
                  <th className="px-3 py-2">To</th>
                  <th className="px-3 py-2 text-right">Drinks</th>
                  <th className="px-3 py-2 text-right">Amount</th>
                </tr>
              </thead>
              <tbody>
                {entries.map((entry) => (
                  <tr
                    key={entry.id}
                    className="bg-berry-100 font-bold text-berry-950 dark:bg-white/10 dark:text-berry-50"
                  >
                    <td className="rounded-l-2xl px-3 py-3">
                      {formatUnixDate(entry.paid_at, 'Unknown')}
                    </td>
                    <td className="px-3 py-3">{entry.payer_display_name}</td>
                    <td className="px-3 py-3">{entry.recipient_display_name}</td>
                    <td className="px-3 py-3 text-right">{entry.drink_count}</td>
                    <td className="rounded-r-2xl px-3 py-3 text-right font-display font-black">
                      {formatEuros(entry.amount_cents)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      ) : (
        <p className="font-bold text-berry-700 dark:text-rose-100">
          No received payments recorded yet.
        </p>
      )}
    </Card>
  )
}

function ExpensesTracker({
  entries,
  summaries,
  loading,
  createPending,
  onCreate,
}: {
  entries: ExpenseEntry[]
  summaries: ExpenseSummary[]
  loading: boolean
  createPending: boolean
  onCreate: (description: string, amountCents: number) => void
}) {
  const [description, setDescription] = useState('')
  const [amount, setAmount] = useState('')
  const [expenseKind, setExpenseKind] = useState<'expense' | 'credit'>('expense')
  const { pushToast } = useToast()

  const submitExpense = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    const parsed = parseEuroAmountToCents(amount)
    const absoluteAmountCents = parsed === null ? null : Math.abs(parsed)
    const trimmed = description.trim()
    if (!trimmed) {
      pushToast({
        tone: 'error',
        title: 'Missing description',
        description: 'Add a short note for the expense.',
      })
      return
    }
    if (absoluteAmountCents === null || absoluteAmountCents === 0) {
      pushToast({
        tone: 'error',
        title: 'Invalid amount',
        description: 'Enter a non-zero EUR amount.',
      })
      return
    }

    onCreate(trimmed, expenseKind === 'expense' ? absoluteAmountCents : -absoluteAmountCents)
    setDescription('')
    setAmount('')
    setExpenseKind('expense')
  }

  return (
    <Card className="mt-5">
      <div className="mb-5 flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
        <div>
          <div className="mb-2 flex items-center gap-3">
            <ReceiptText className="size-6 text-primary dark:text-blush" />
            <p className="font-display text-xl font-black">Expenses</p>
          </div>
        </div>

        <form className="grid gap-2 sm:grid-cols-[1fr_8rem_9rem_auto] lg:w-[48rem]" onSubmit={submitExpense}>
          <input
            className="min-h-12 rounded-2xl border-2 border-primary/10 bg-[#fffdf8] px-3 font-bold text-berry-950 outline-none transition placeholder:text-berry-700/70 focus:border-primary dark:bg-berry-950 dark:text-rose-50 dark:placeholder:text-rose-100/60"
            value={description}
            onChange={(event) => setDescription(event.target.value)}
            placeholder="Description"
            maxLength={180}
          />
          <select
            className="min-h-12 rounded-2xl border-2 border-primary/10 bg-[#fffdf8] px-3 font-bold text-berry-950 outline-none transition focus:border-primary dark:bg-berry-950 dark:text-rose-50"
            value={expenseKind}
            onChange={(event) => setExpenseKind(event.target.value as 'expense' | 'credit')}
          >
            <option value="expense">Expense +</option>
            <option value="credit">Refund / Pfand -</option>
          </select>
          <input
            className="min-h-12 rounded-2xl border-2 border-primary/10 bg-[#fffdf8] px-3 font-bold text-berry-950 outline-none transition placeholder:text-berry-700/70 focus:border-primary dark:bg-berry-950 dark:text-rose-50 dark:placeholder:text-rose-100/60"
            value={amount}
            onChange={(event) => setAmount(event.target.value)}
            placeholder="12,50"
            inputMode="decimal"
          />
          <Button type="submit" disabled={createPending}>
            <Plus className="size-4" />
            Add
          </Button>
        </form>
      </div>

      {loading ? (
        <Skeleton className="h-72" />
      ) : (
        <div className="space-y-6">
          <ExpenseSummaries summaries={summaries} />
          <ExpenseEntries entries={entries} />
        </div>
      )}
    </Card>
  )
}

function ExpenseSummaries({
  summaries,
}: {
  summaries: ExpenseSummary[]
}) {
  if (!summaries.length) {
    return (
      <p className="font-bold text-berry-700 dark:text-rose-100">
        No staff expense balances yet.
      </p>
    )
  }

  return (
    <div>
      <p className="mb-3 font-display text-lg font-black">Staff balances</p>
      <div className="space-y-3 md:hidden">
        {summaries.map((summary) => (
          <div
            key={summary.user_id}
            className="rounded-3xl bg-berry-100 p-4 font-bold text-berry-950 dark:bg-white/10 dark:text-berry-50"
          >
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <p className="break-words font-display text-lg font-black">
                  {summary.display_name}
                </p>
              </div>
              <p className="shrink-0 font-display text-xl font-black text-primary dark:text-blush">
                {formatEuros(summary.outstanding_amount_cents)}
              </p>
            </div>
          </div>
        ))}
      </div>

      <div className="hidden overflow-x-auto md:block">
        <table className="w-full min-w-[30rem] border-separate border-spacing-y-2 text-left">
          <thead>
            <tr className="text-xs font-black uppercase tracking-[0.14em] text-berry-700 dark:text-rose-100">
              <th className="px-3 py-2">Staff member</th>
              <th className="px-3 py-2 text-right">Balance</th>
            </tr>
          </thead>
          <tbody>
            {summaries.map((summary) => (
              <tr
                key={summary.user_id}
                className="bg-berry-100 font-bold text-berry-950 dark:bg-white/10 dark:text-berry-50"
              >
                <td className="rounded-l-2xl px-3 py-3 font-display font-black">
                  {summary.display_name}
                </td>
                <td className="rounded-r-2xl px-3 py-3 text-right font-display font-black">
                  {formatEuros(summary.outstanding_amount_cents)}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}

function ExpenseEntries({ entries }: { entries: ExpenseEntry[] }) {
  if (!entries.length) {
    return (
      <p className="font-bold text-berry-700 dark:text-rose-100">
        No expense entries logged yet.
      </p>
    )
  }

  return (
    <div>
      <p className="mb-3 font-display text-lg font-black">Expense ledger</p>
      <div className="space-y-3 md:hidden">
        {entries.map((entry) => (
          <div
            key={entry.id}
            className="rounded-3xl bg-berry-100 p-4 font-bold text-berry-950 dark:bg-white/10 dark:text-berry-50"
          >
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <p className="break-words font-display text-lg font-black">
                  {entry.description}
                </p>
                <p className="text-xs font-black uppercase text-berry-700 dark:text-rose-100">
                  {entry.display_name} · {formatUnixDate(entry.logged_at, 'Unknown')}
                </p>
              </div>
              <p className="shrink-0 font-display text-xl font-black text-primary dark:text-blush">
                {formatEuros(entry.amount_cents)}
              </p>
            </div>
          </div>
        ))}
      </div>

      <div className="hidden overflow-x-auto md:block">
        <table className="w-full min-w-[46rem] border-separate border-spacing-y-2 text-left">
          <thead>
            <tr className="text-xs font-black uppercase tracking-[0.14em] text-berry-700 dark:text-rose-100">
              <th className="px-3 py-2">Date</th>
              <th className="px-3 py-2">Staff member</th>
              <th className="px-3 py-2">Description</th>
              <th className="px-3 py-2 text-right">Amount</th>
            </tr>
          </thead>
          <tbody>
            {entries.map((entry) => (
              <tr
                key={entry.id}
                className="bg-berry-100 font-bold text-berry-950 dark:bg-white/10 dark:text-berry-50"
              >
                <td className="rounded-l-2xl px-3 py-3">
                  {formatUnixDate(entry.logged_at, 'Unknown')}
                </td>
                <td className="px-3 py-3">{entry.display_name}</td>
                <td className="px-3 py-3">{entry.description}</td>
                <td className="rounded-r-2xl px-3 py-3 text-right font-display font-black">
                  {formatEuros(entry.amount_cents)}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}

function parseEuroAmountToCents(value: string) {
  const normalized = value.trim().replace(',', '.')
  if (!normalized) return null
  const amount = Number(normalized)
  if (!Number.isFinite(amount)) return null
  return Math.round(amount * 100)
}
