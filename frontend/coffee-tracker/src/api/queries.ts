import { type QueryClient, useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  api,
  type CreateExpenseRequest,
  type CraftResponse,
  type EquipRequest,
  type LockerResponse,
  type LootboxOpenResponse,
  type EmailPaymentReminderRequest,
  type GlobalRange,
  type MarkPaidRequest,
  type PaymentReminderRequest,
  type UpdateSettingsRequest,
  type UpdateStockRequest,
  type WheelSpinResponse,
} from './client'
import { useSpinInFlight } from '../lib/spinGate'

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
  locker: ['locker'] as const,
  chores: ['chores'] as const,
  wheel: ['wheel'] as const,
  wheelStats: ['wheel', 'stats'] as const,
  stampCard: (month: string) => ['stempelkarte', month] as const,
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

function invalidatePaymentData(queryClient: QueryClient) {
  return Promise.all([
    queryClient.invalidateQueries({ queryKey: queryKeys.payments }),
    queryClient.invalidateQueries({ queryKey: queryKeys.myPayment }),
    queryClient.invalidateQueries({ queryKey: queryKeys.expenses }),
    queryClient.invalidateQueries({ queryKey: queryKeys.globalStats }),
    queryClient.invalidateQueries({ queryKey: queryKeys.achievements }),
  ])
}

export function useMarkPaidMutation() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (body: MarkPaidRequest) => api.markPaid(body),
    onSuccess: () => invalidatePaymentData(queryClient),
  })
}

// Recipient confirms (or rejects) a payment someone marked as paid to them.
export function useReviewPaymentMutation() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ id, accept }: { id: string; accept: boolean }) =>
      accept ? api.confirmPayment(id).then(() => undefined) : api.rejectPayment(id),
    onSuccess: () => invalidatePaymentData(queryClient),
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
        // Shopping trips earn a lootbox and may unlock achievements.
        queryClient.invalidateQueries({ queryKey: queryKeys.locker }),
        queryClient.invalidateQueries({ queryKey: queryKeys.achievements }),
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

export function useStampCard(month: string) {
  return useQuery({
    queryKey: queryKeys.stampCard(month),
    queryFn: () => api.stampCard(month),
  })
}

export function invalidateCoffeeData(queryClient: ReturnType<typeof useQueryClient>) {
  return Promise.all([
    queryClient.invalidateQueries({ queryKey: ['stats'] }),
    queryClient.invalidateQueries({ queryKey: ['stempelkarte'] }),
    queryClient.invalidateQueries({ queryKey: queryKeys.achievements }),
    queryClient.invalidateQueries({ queryKey: queryKeys.locker }),
    queryClient.invalidateQueries({ queryKey: queryKeys.leaderboard }),
    queryClient.invalidateQueries({ queryKey: queryKeys.coffeeTwins }),
    queryClient.invalidateQueries({ queryKey: ['payments'] }),
    queryClient.invalidateQueries({ queryKey: queryKeys.expenses }),
    queryClient.invalidateQueries({ queryKey: queryKeys.stock }),
    // A logged drink may use a free drink won on the wheel.
    queryClient.invalidateQueries({ queryKey: queryKeys.wheel }),
  ])
}

export function useLocker(enabled = true) {
  // Hold still while a wheel spin is in the air (it may have granted crates).
  const spinInFlight = useSpinInFlight()
  return useQuery({
    queryKey: queryKeys.locker,
    queryFn: api.locker,
    enabled: enabled && !spinInFlight,
    staleTime: 60_000,
  })
}

/** Writes the fresh locker into the cache and refreshes what the change affects. */
export function useLockerUpdate() {
  const queryClient = useQueryClient()
  return (data: { locker: LockerResponse } | LockerResponse, touchesAchievements = false) => {
    queryClient.setQueryData(queryKeys.locker, 'locker' in data ? data.locker : data)
    void queryClient.invalidateQueries({ queryKey: queryKeys.leaderboard })
    void queryClient.invalidateQueries({ queryKey: queryKeys.coffeeTwins })
    if (touchesAchievements) {
      void queryClient.invalidateQueries({ queryKey: queryKeys.achievements })
    }
  }
}

/**
 * Opens a lootbox. Deliberately does not touch the cache: the reel animation
 * reveals the prize first, then the caller applies the result with
 * `useLockerUpdate` so the collection does not spoil it.
 */
export function useOpenLootboxMutation() {
  return useMutation({ mutationFn: (): Promise<LootboxOpenResponse> => api.openLootbox() })
}

export function useCraftMutation() {
  const update = useLockerUpdate()
  return useMutation({
    mutationFn: (itemId: string) => api.craftCosmetic(itemId),
    onSuccess: (data: CraftResponse) => update(data, data.unlocked_achievements.length > 0),
  })
}

export function useEquipMutation() {
  const update = useLockerUpdate()
  return useMutation({
    mutationFn: (body: EquipRequest) => api.equipCosmetic(body),
    onSuccess: (data) => update(data),
  })
}

/** Lucky wheel status (members only). */
export function useWheel(enabled = true) {
  // The draw is stored before the wheel lands; don't reveal it through a refetch.
  const spinInFlight = useSpinInFlight()
  return useQuery({
    queryKey: queryKeys.wheel,
    queryFn: api.wheel,
    enabled: enabled && !spinInFlight,
    staleTime: 60_000,
  })
}

/**
 * Spins the lucky wheel. Like lootbox opening, the cache is left alone until
 * the wheel has landed; then the caller applies the result with
 * `useWheelSpinApply` so nothing spoils the prize.
 */
export function useSpinWheelMutation() {
  return useMutation({
    mutationFn: ({ paid, spinId }: { paid: boolean; spinId: string }): Promise<WheelSpinResponse> =>
      api.spinWheel(paid, spinId),
  })
}

export function useWheelSpinApply() {
  const queryClient = useQueryClient()
  return (data: WheelSpinResponse) => {
    queryClient.setQueryData(queryKeys.wheel, data.status)
    void queryClient.invalidateQueries({ queryKey: queryKeys.wheelStats })
    void queryClient.invalidateQueries({ queryKey: queryKeys.locker })
    void queryClient.invalidateQueries({ queryKey: ['payments'] })
    void queryClient.invalidateQueries({ queryKey: queryKeys.globalStats })
    if (data.unlocked_achievements.length > 0) {
      void queryClient.invalidateQueries({ queryKey: queryKeys.achievements })
    }
  }
}

export function useWheelStats(enabled = true) {
  return useQuery({ queryKey: queryKeys.wheelStats, queryFn: api.wheelStats, enabled })
}

export function useChores() {
  return useQuery({ queryKey: queryKeys.chores, queryFn: api.chores })
}

export function useLogChoreMutation() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (chore: string) => api.logChore(chore),
    onSuccess: (data) => {
      queryClient.setQueryData(queryKeys.chores, data.board)
      void queryClient.invalidateQueries({ queryKey: queryKeys.locker })
      void queryClient.invalidateQueries({ queryKey: queryKeys.achievements })
    },
  })
}
