import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  api,
  type CreateExpenseRequest,
  type EmailPaymentReminderRequest,
  type GlobalRange,
  type MarkPaidRequest,
  type PaymentReminderRequest,
  type UpdateSettingsRequest,
  type UpdateStockRequest,
} from './client'

export const queryKeys = {
  me: ['me'] as const,
  machines: ['machines'] as const,
  beverages: ['beverages'] as const,
  summary: ['stats', 'summary'] as const,
  buckets: ['stats', 'buckets'] as const,
  hourly: ['stats', 'hourly'] as const,
  myHourly: (range: GlobalRange) => ['stats', 'hourly', range] as const,
  myBeverages: (range: GlobalRange) => ['stats', 'my-beverages', range] as const,
  caffeine: ['stats', 'caffeine'] as const,
  streaks: ['stats', 'streaks'] as const,
  globalStats: ['stats', 'global'] as const,
  globalHourly: (range: GlobalRange) => ['stats', 'global-hourly', range] as const,
  globalBuckets: (range: GlobalRange) => ['stats', 'global-buckets', range] as const,
  globalBeverages: (range: GlobalRange) => ['stats', 'global-beverages', range] as const,
  achievements: ['achievements'] as const,
  leaderboard: ['leaderboard'] as const,
  coffeeTwins: ['coffeeTwins'] as const,
  payments: ['payments'] as const,
  myPayment: ['payments', 'me'] as const,
  paymentConfig: ['payments', 'config'] as const,
  paymentsLinks: ['payments', 'links'] as const,
  paymentRecipients: ['payments', 'recipients'] as const,
  expenses: ['expenses'] as const,
  stock: ['stock'] as const,
  version: ['version'] as const,
}

export function useMe() {
  return useQuery({ queryKey: queryKeys.me, queryFn: api.me, retry: false })
}

export function useCatalog() {
  const machines = useQuery({
    queryKey: queryKeys.machines,
    queryFn: api.machines,
  })
  const beverages = useQuery({
    queryKey: queryKeys.beverages,
    queryFn: api.beverages,
  })
  return { machines, beverages }
}

export function useStats() {
  return {
    summary: useQuery({ queryKey: queryKeys.summary, queryFn: api.summary }),
    buckets: useQuery({ queryKey: queryKeys.buckets, queryFn: api.buckets }),
    hourly: useQuery({ queryKey: queryKeys.hourly, queryFn: () => api.hourly() }),
    caffeine: useQuery({
      queryKey: queryKeys.caffeine,
      queryFn: api.caffeine,
      refetchInterval: 60_000,
    }),
    streaks: useQuery({ queryKey: queryKeys.streaks, queryFn: api.streaks }),
    globalStats: useQuery({
      queryKey: queryKeys.globalStats,
      queryFn: api.globalStats,
    }),
  }
}

export function useMyHourly(range: GlobalRange) {
  return useQuery({
    queryKey: queryKeys.myHourly(range),
    queryFn: () => api.hourly(range),
  })
}

export function useMyBeverages(range: GlobalRange) {
  return useQuery({
    queryKey: queryKeys.myBeverages(range),
    queryFn: () => api.myBeverages(range),
  })
}

export function useGlobalHourly(range: GlobalRange) {
  return useQuery({
    queryKey: queryKeys.globalHourly(range),
    queryFn: () => api.globalHourly(range),
  })
}

export function useGlobalBuckets(range: GlobalRange) {
  return useQuery({
    queryKey: queryKeys.globalBuckets(range),
    queryFn: () => api.globalBuckets(range),
  })
}

export function useGlobalBeverages(range: GlobalRange) {
  return useQuery({
    queryKey: queryKeys.globalBeverages(range),
    queryFn: () => api.globalBeverages(range),
  })
}

export function useAchievements() {
  return useQuery({
    queryKey: queryKeys.achievements,
    queryFn: api.achievements,
  })
}

export function useLeaderboard() {
  return useQuery({
    queryKey: queryKeys.leaderboard,
    queryFn: api.leaderboard,
  })
}

export function useCoffeeTwins() {
  return useQuery({
    queryKey: queryKeys.coffeeTwins,
    queryFn: api.coffeeTwins,
  })
}

export function useSettingsMutation() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (body: UpdateSettingsRequest) => api.updateSettings(body),
    onSuccess: (data) => queryClient.setQueryData(queryKeys.me, data),
  })
}

export function usePayments(enabled = true) {
  return useQuery({
    queryKey: queryKeys.payments,
    queryFn: api.payments,
    enabled,
  })
}

export function useMyPayment() {
  return useQuery({
    queryKey: queryKeys.myPayment,
    queryFn: api.myPayment,
  })
}

export function usePaymentConfig(enabled = true) {
  return useQuery({
    queryKey: queryKeys.paymentConfig,
    queryFn: api.paymentConfig,
    enabled,
    staleTime: 5 * 60_000,
  })
}

export function usePaymentsLinks(enabled = true) {
  return useQuery({
    queryKey: queryKeys.paymentsLinks,
    queryFn: api.paymentsLinks,
    enabled,
    staleTime: 5 * 60_000,
  })
}

export function usePaymentRecipients(enabled = true) {
  return useQuery({
    queryKey: queryKeys.paymentRecipients,
    queryFn: api.paymentRecipients,
    enabled,
    staleTime: 5 * 60_000,
  })
}

export function useMarkPaidMutation() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (body: MarkPaidRequest) => api.markPaid(body),
    onSuccess: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.payments }),
        queryClient.invalidateQueries({ queryKey: queryKeys.myPayment }),
        queryClient.invalidateQueries({ queryKey: queryKeys.expenses }),
        queryClient.invalidateQueries({ queryKey: queryKeys.globalStats }),
        queryClient.invalidateQueries({ queryKey: queryKeys.achievements }),
      ]),
  })
}

export function usePaymentReminderMutation() {
  return useMutation({
    mutationFn: (body: PaymentReminderRequest) => api.sendPaymentReminder(body),
  })
}

export function useEmailPaymentReminderMutation() {
  return useMutation({
    mutationFn: (body: EmailPaymentReminderRequest) =>
      api.sendEmailPaymentReminder(body),
  })
}

export function useExpenses(enabled = true) {
  return useQuery({
    queryKey: queryKeys.expenses,
    queryFn: api.expenses,
    enabled,
  })
}

export function useCreateExpenseMutation() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (body: CreateExpenseRequest) => api.createExpense(body),
    onSuccess: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.expenses }),
        queryClient.invalidateQueries({ queryKey: queryKeys.globalStats }),
      ]),
  })
}

export function useStock() {
  return useQuery({
    queryKey: queryKeys.stock,
    queryFn: api.stock,
  })
}

export function useUpdateStockMutation() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({
      itemId,
      body,
    }: {
      itemId: string
      body: UpdateStockRequest
    }) => api.updateStock(itemId, body),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: queryKeys.stock }),
  })
}

export function useVersion() {
  return useQuery({
    queryKey: queryKeys.version,
    queryFn: api.version,
    staleTime: 60 * 60_000,
    retry: false,
  })
}

export function invalidateCoffeeData(queryClient: ReturnType<typeof useQueryClient>) {
  return Promise.all([
    queryClient.invalidateQueries({ queryKey: ['stats'] }),
    queryClient.invalidateQueries({ queryKey: queryKeys.achievements }),
    queryClient.invalidateQueries({ queryKey: queryKeys.leaderboard }),
    queryClient.invalidateQueries({ queryKey: queryKeys.coffeeTwins }),
    queryClient.invalidateQueries({ queryKey: ['payments'] }),
    queryClient.invalidateQueries({ queryKey: queryKeys.expenses }),
    queryClient.invalidateQueries({ queryKey: queryKeys.stock }),
  ])
}
