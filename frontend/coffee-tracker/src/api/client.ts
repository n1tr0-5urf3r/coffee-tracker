import type { components } from './schema'
import { runtimeConfig } from '../config/runtime'

export type CurrentUser = components['schemas']['CurrentUser']
export type MeResponse = components['schemas']['MeResponse']
export type GuestLoginRequest = components['schemas']['GuestLoginRequest']
export type UpdateSettingsRequest =
  components['schemas']['UpdateSettingsRequest']
export type Machine = components['schemas']['Machine']
export type Beverage = components['schemas']['Beverage']
export type BeverageSummary = components['schemas']['BeverageSummary']
export type ConsumptionResponse = components['schemas']['ConsumptionResponse']
export type SummaryResponse = components['schemas']['SummaryResponse']
export type BucketsResponse = components['schemas']['BucketsResponse']
export type HourlyResponse = components['schemas']['HourlyResponse']
export type CaffeineResponse = components['schemas']['CaffeineResponse']
export type StreaksResponse = components['schemas']['StreaksResponse']
export type GlobalStatsResponse = components['schemas']['GlobalStatsResponse']
export type GlobalHourlyResponse = components['schemas']['GlobalHourlyResponse']
export type GlobalBucketsResponse =
  components['schemas']['GlobalBucketsResponse']
export type GlobalBeveragesResponse = {
  beverages: BeverageSummary[]
}
export type MyBeveragesResponse = components['schemas']['MyBeverageResponse']
export type GlobalRange = '30' | 'all'
export type AchievementsResponse = components['schemas']['AchievementsResponse']
export type AchievementItem = components['schemas']['AchievementItem']
export type Stamp = components['schemas']['Stamp']
export type StampCard = components['schemas']['StampCard']
export type StampCardResponse = components['schemas']['StampCardResponse']
export type LeaderboardResponse = components['schemas']['LeaderboardResponse']
export type CoffeeTwinsResponse = components['schemas']['CoffeeTwinsResponse']
export type PaymentSummary = components['schemas']['PaymentSummary']
export type PaymentsResponse = components['schemas']['PaymentsResponse']
export type MyPaymentResponse = components['schemas']['MyPaymentResponse']
export type PaymentConfigResponse =
  components['schemas']['PaymentConfigResponse']
export type PaymentRecord = components['schemas']['PaymentRecord']
export type ReceivedPaymentEntry =
  components['schemas']['ReceivedPaymentEntry']
export type MarkPaidRequest = components['schemas']['MarkPaidRequest']
export type PaymentReminderRequest =
  components['schemas']['PaymentReminderRequest']
export type PaymentReminderResponse =
  components['schemas']['PaymentReminderResponse']
export type EmailPaymentReminderRequest =
  components['schemas']['EmailPaymentReminderRequest']
export type EmailPaymentReminderResponse =
  components['schemas']['EmailPaymentReminderResponse']
export type PaypalLinkEntry = components['schemas']['PaypalLinkEntry']
export type PaypalLinksResponse = components['schemas']['PaypalLinksResponse']
export type PaymentRecipient = components['schemas']['PaymentRecipient']
export type PaymentRecipientsResponse =
  components['schemas']['PaymentRecipientsResponse']
export type ExpenseEntry = components['schemas']['ExpenseEntry']
export type ExpenseSummary = components['schemas']['ExpenseSummary']
export type ExpensesResponse = components['schemas']['ExpensesResponse']
export type CreateExpenseRequest =
  components['schemas']['CreateExpenseRequest']
export type StockItem = components['schemas']['StockItem']
export type StockResponse = components['schemas']['StockResponse']
export type UpdateStockRequest = components['schemas']['UpdateStockRequest']
export type VersionResponse = components['schemas']['VersionResponse']

const apiBaseUrl = runtimeConfig.API_BASE_URL ?? import.meta.env.VITE_API_BASE_URL ?? ''

export class ApiError extends Error {
  status: number
  body: unknown

  constructor(status: number, message: string, body: unknown) {
    super(message)
    this.name = 'ApiError'
    this.status = status
    this.body = body
  }
}

export function isBackendUnavailableError(error: unknown) {
  const status =
    error instanceof ApiError
      ? error.status
      : typeof error === 'object' && error !== null && 'status' in error
        ? Number((error as { status: unknown }).status)
        : Number.NaN
  if ([0, 500, 502, 503, 504].includes(status)) {
    return true
  }

  const message = error instanceof Error ? error.message : String(error)
  return /backend is currently unavailable|bad gateway|econnrefused|failed to fetch|networkerror/i.test(
    message,
  )
}

type JsonBody = Record<string, unknown> | undefined

async function request<T>(
  path: string,
  options: RequestInit & { json?: JsonBody } = {},
): Promise<T> {
  const headers = new Headers(options.headers)
  if (options.json) {
    headers.set('content-type', 'application/json')
  }

  let response: Response
  try {
    response = await fetch(`${apiBaseUrl}${path}`, {
      ...options,
      headers,
      credentials: 'include',
      body: options.json ? JSON.stringify(options.json) : options.body,
    })
  } catch (error) {
    throw new ApiError(
      0,
      'Backend is currently unavailable. Please try again in a moment.',
      error,
    )
  }

  if (response.status === 401) {
    window.dispatchEvent(new CustomEvent('coffee:unauthorized'))
  }

  if (!response.ok) {
    const text = await response.text().catch(() => '')
    let body: unknown = text
    if (text) {
      try {
        body = JSON.parse(text)
      } catch {
        body = text
      }
    }
    const message =
      body && typeof body === 'object' && 'error' in body
        ? String((body as { error: unknown }).error)
        : `Request failed with ${response.status}`
    throw new ApiError(response.status, message, body)
  }

  if (response.status === 204) {
    return undefined as T
  }

  return response.json() as Promise<T>
}

export const api = {
  gitlabLoginUrl: `${apiBaseUrl}/api/v1/auth/gitlab/login`,
  authentikLoginUrl: `${apiBaseUrl}/api/v1/auth/authentik/login`,
  guestLogin: (body: GuestLoginRequest) =>
    request<MeResponse>('/api/v1/auth/guest', {
      method: 'POST',
      json: body as unknown as JsonBody,
    }),
  logout: () => request<void>('/api/v1/auth/logout', { method: 'POST' }),
  me: () => request<MeResponse>('/api/v1/me'),
  resetGuestRecoveryCode: () =>
    request<MeResponse>('/api/v1/me/guest-recovery-code', { method: 'POST' }),
  updateSettings: (body: UpdateSettingsRequest) =>
    request<MeResponse>('/api/v1/me/settings', {
      method: 'PATCH',
      json: body as unknown as JsonBody,
    }),
  machines: () => request<{ machines: Machine[] }>('/api/v1/machines'),
  beverages: () => request<{ beverages: Beverage[] }>('/api/v1/beverages'),
  createConsumption: (body: { machine_id: string; beverage_id: string }) =>
    request<ConsumptionResponse>('/api/v1/consumptions', {
      method: 'POST',
      json: body,
    }),
  deleteConsumption: (id: string) =>
    request<void>(`/api/v1/consumptions/${encodeURIComponent(id)}`, {
      method: 'DELETE',
    }),
  summary: () => request<SummaryResponse>('/api/v1/stats/me/summary'),
  buckets: () => request<BucketsResponse>('/api/v1/stats/me/buckets'),
  hourly: (range?: GlobalRange) =>
    request<HourlyResponse>(
      range
        ? `/api/v1/stats/me/hourly?range=${encodeURIComponent(range)}`
        : '/api/v1/stats/me/hourly',
    ),
  myBeverages: (range: GlobalRange) =>
    request<MyBeveragesResponse>(
      `/api/v1/stats/me/beverages?range=${encodeURIComponent(range)}`,
    ),
  caffeine: () => request<CaffeineResponse>('/api/v1/stats/me/caffeine'),
  streaks: () => request<StreaksResponse>('/api/v1/stats/me/streaks'),
  globalStats: () =>
    request<GlobalStatsResponse>('/api/v1/stats/global/summary'),
  globalHourly: (range: GlobalRange) =>
    request<GlobalHourlyResponse>(
      `/api/v1/stats/global/hourly?range=${encodeURIComponent(range)}`,
    ),
  globalBuckets: (range: GlobalRange) =>
    request<GlobalBucketsResponse>(
      `/api/v1/stats/global/buckets?range=${encodeURIComponent(range)}`,
    ),
  globalBeverages: (range: GlobalRange) =>
    request<GlobalBeveragesResponse>(
      `/api/v1/stats/global/beverages?range=${encodeURIComponent(range)}`,
    ),
  achievements: () =>
    request<AchievementsResponse>('/api/v1/achievements/me'),
  stampCard: (month?: string) =>
    request<StampCardResponse>(
      month
        ? `/api/v1/stempelkarte/me?month=${encodeURIComponent(month)}`
        : '/api/v1/stempelkarte/me',
    ),
  leaderboard: () => request<LeaderboardResponse>('/api/v1/leaderboard'),
  coffeeTwins: () => request<CoffeeTwinsResponse>('/api/v1/coffee-twins'),
  payments: () => request<PaymentsResponse>('/api/v1/payments'),
  myPayment: () => request<MyPaymentResponse>('/api/v1/payments/me'),
  paymentConfig: () => request<PaymentConfigResponse>('/api/v1/payments/config'),
  paymentsLinks: () => request<PaypalLinksResponse>('/api/v1/payments/links'),
  paymentRecipients: () =>
    request<PaymentRecipientsResponse>('/api/v1/payments/recipients'),
  markPaid: (body: MarkPaidRequest) =>
    request<PaymentRecord>('/api/v1/payments', {
      method: 'POST',
      json: body as unknown as JsonBody,
    }),
  sendPaymentReminder: (body: PaymentReminderRequest) =>
    request<PaymentReminderResponse>('/api/v1/payments/reminders', {
      method: 'POST',
      json: body as unknown as JsonBody,
    }),
  sendEmailPaymentReminder: (body: EmailPaymentReminderRequest) =>
    request<EmailPaymentReminderResponse>('/api/v1/payments/email-reminders', {
      method: 'POST',
      json: body as unknown as JsonBody,
    }),
  expenses: () => request<ExpensesResponse>('/api/v1/expenses'),
  createExpense: (body: CreateExpenseRequest) =>
    request<ExpenseEntry>('/api/v1/expenses', {
      method: 'POST',
      json: body as unknown as JsonBody,
    }),
  stock: () => request<StockResponse>('/api/v1/stock'),
  updateStock: (itemId: string, body: UpdateStockRequest) =>
    request<StockItem>(`/api/v1/stock/${encodeURIComponent(itemId)}`, {
      method: 'PATCH',
      json: body as unknown as JsonBody,
    }),
  version: () => request<VersionResponse>('/api/v1/version'),
}
