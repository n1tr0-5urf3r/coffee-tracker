import { Copy, HelpCircle, KeyRound, LogOut, Moon, Package, Save, Sun, SunMoon } from 'lucide-react'
import { useEffect, useState, type ReactNode } from 'react'
import { useNavigate } from 'react-router-dom'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { api, isBackendUnavailableError } from '../api/client'
import {
  queryKeys,
  useMe,
  useSettingsMutation,
  useStock,
  useUpdateStockMutation,
} from '../api/queries'
import { Button } from '../components/Button'
import { Card } from '../components/Card'
import { LogoMark } from '../components/LogoMark'
import { PageHeader } from '../components/PageHeader'
import { useTheme, type ThemeChoice } from '../lib/theme'
import { useToast } from '../lib/toast'
import { formatStockValue } from '../lib/stock'
import {
  formatGuestRecoveryCode,
  getGuestRecoveryCode,
  storeGuestRecoveryCode,
} from '../lib/guestRecovery'
import { branding } from '../config/branding'

export function SettingsPage() {
  const { data } = useMe()
  const settings = useSettingsMutation()
  const { theme, setTheme } = useTheme()
  const { pushToast } = useToast()
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const user = data?.user
  const isStaff = Boolean(user?.stock_manager)
  const [guestRecoveryCode, setGuestRecoveryCode] = useState(() => getGuestRecoveryCode())
  const resetGuestRecoveryCode = useMutation({
    mutationFn: api.resetGuestRecoveryCode,
    onSuccess: (response) => {
      storeGuestRecoveryCode(response.guest_recovery_code, response.user.display_name)
      setGuestRecoveryCode(response.guest_recovery_code ?? '')
      queryClient.setQueryData(queryKeys.me, response)
      pushToast({
        tone: 'success',
        title: 'New guest code created',
        description: 'Use the new code on another device to continue this guest tab.',
      })
    },
    onError: () =>
      pushToast({
        tone: 'error',
        title: 'Guest code not created',
        description: 'Please try again in a moment.',
      }),
  })

  const toggle = (
    key: 'leaderboard_opt_in' | 'mattermost_opt_in' | 'achievement_sharing_opt_out',
    value: boolean,
  ) => {
    settings.mutate(
      { [key]: value },
      {
        onSuccess: () =>
          pushToast({
            tone: 'success',
            title: 'Settings saved',
            description: 'Your coffee preferences are up to date.',
          }),
      },
    )
  }

  const [paypalDraft, setPaypalDraft] = useState<string>(user?.paypal_link ?? '')

  useEffect(() => {
    setPaypalDraft(user?.paypal_link ?? '')
  }, [user?.paypal_link])

  useEffect(() => {
    if (!user?.display_name) return
    setGuestRecoveryCode(getGuestRecoveryCode(user.display_name))
  }, [user?.display_name])

  useEffect(() => {
    if (!data?.guest_recovery_code) return
    storeGuestRecoveryCode(data.guest_recovery_code, data.user.display_name)
    setGuestRecoveryCode(data.guest_recovery_code)
  }, [data?.guest_recovery_code, data?.user.display_name])

  const savePaypal = () => {
    const trimmed = paypalDraft.trim()
    settings.mutate(
      { paypal_link: trimmed === '' ? null : trimmed },
      {
        onSuccess: () => {
          queryClient.invalidateQueries({ queryKey: queryKeys.paymentsLinks })
          pushToast({
            tone: 'success',
            title: 'Settings saved',
            description: 'Your PayPal link was updated.',
          })
        },
        onError: (error) =>
          pushToast({
            tone: 'error',
            title: 'PayPal link not saved',
            description: isBackendUnavailableError(error)
              ? 'Backend is currently unavailable. Please try again in a moment.'
              : error instanceof Error
                ? error.message
                : 'Please enter a valid https payment link.',
          }),
      },
    )
  }

  const logout = async () => {
    await api.logout()
    queryClient.clear()
    navigate('/login', { replace: true })
  }

  const copyGuestRecoveryCode = async () => {
    if (!guestRecoveryCode) return
    await navigator.clipboard.writeText(guestRecoveryCode)
    pushToast({
      tone: 'success',
      title: 'Guest code copied',
      description: 'Use it on another device to continue the same guest tab.',
    })
  }

  return (
    <div>
      <PageHeader eyebrow="Preferences" title="Settings" />
      <div className="space-y-5">
        <div className="grid gap-5 xl:grid-cols-2">
          <Card className="flex min-h-28 items-center gap-4 p-4 sm:p-5">
            <LogoMark label={false} className="shrink-0 scale-75" />
            <div className="min-w-0">
              <p className="break-words font-display text-xl font-black">
                {user?.display_name}
              </p>
              <p className="mt-1 inline-flex rounded-full bg-rose-soft px-3 py-1 text-xs font-black text-primary dark:bg-white/10 dark:text-blush">
                {user?.user_type === 'member' ? 'Member account' : 'Guest session'}
              </p>
            </div>
          </Card>

          <Card>
            <p className="mb-4 font-display text-xl font-black">Theme</p>
            <div className="grid gap-3 sm:grid-cols-3 xl:grid-cols-1 2xl:grid-cols-3">
              <ThemeButton
                value="system"
                current={theme}
                onSelect={setTheme}
                icon={<SunMoon className="size-5" />}
                label="System"
              />
              <ThemeButton
                value="light"
                current={theme}
                onSelect={setTheme}
                icon={<Sun className="size-5" />}
                label="Light"
              />
              <ThemeButton
                value="dark"
                current={theme}
                onSelect={setTheme}
                icon={<Moon className="size-5" />}
                label="Dark"
              />
            </div>
          </Card>
        </div>

        {user?.user_type === 'guest' ? (
          <Card className="p-4 sm:p-5">
            <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
              <div className="min-w-0">
                <p className="mb-1 flex items-center gap-2 font-display text-lg font-black">
                  <KeyRound className="size-5 text-primary dark:text-blush" />
                  Guest code
                  <span className="group relative inline-flex">
                    <HelpCircle
                      className="size-4 text-berry-700 dark:text-rose-100"
                      aria-label="Guest code help"
                    />
                    <span className="pointer-events-none absolute left-1/2 top-7 z-20 hidden w-64 -translate-x-1/2 rounded-2xl border border-white/10 bg-berry-950 px-3 py-2 text-xs font-bold leading-snug text-white shadow-soft group-hover:block group-focus-within:block dark:bg-berry-950 dark:text-berry-50">
                      Use this code with the same guest name on another device to keep the same debt and history.
                    </span>
                  </span>
                </p>
                <p className="text-xs font-bold text-berry-700 dark:text-rose-100">
                  Creating a new code replaces the old one.
                </p>
              </div>
              <div className="flex flex-col gap-2 sm:items-end">
                {guestRecoveryCode ? (
                  <code className="rounded-2xl bg-berry-950 px-3 py-2 font-display text-sm font-black tracking-[0.14em] text-white dark:bg-white dark:text-berry-950">
                    {formatGuestRecoveryCode(guestRecoveryCode)}
                  </code>
                ) : null}
                <div className="flex flex-wrap gap-2 sm:justify-end">
                  {guestRecoveryCode ? (
                    <Button variant="secondary" onClick={copyGuestRecoveryCode}>
                      <Copy className="size-4" />
                      Copy
                    </Button>
                  ) : null}
                  <Button
                    variant={guestRecoveryCode ? 'ghost' : 'secondary'}
                    onClick={() => resetGuestRecoveryCode.mutate()}
                    disabled={resetGuestRecoveryCode.isPending}
                  >
                    <Copy className="size-4" />
                    {guestRecoveryCode ? 'New code' : 'Generate code'}
                  </Button>
                </div>
              </div>
            </div>
          </Card>
        ) : null}

        <Card>
            <p className="mb-4 font-display text-xl font-black">Sharing</p>
            <ToggleRow
              label="Leaderboard opt-in"
              description={`Appear in the all-time ${branding.organizationShortName} coffee rankings.`}
              checked={Boolean(user?.leaderboard_opt_in)}
              onChange={(checked) => toggle('leaderboard_opt_in', checked)}
            />
            <ToggleRow
              label="Webhook achievement pings"
              description={
                isStaff
                  ? 'Let unlock celebrations post when the backend webhook is configured.'
                  : `Available to ${branding.staffLabel} only.`
              }
              checked={isStaff && Boolean(user?.mattermost_opt_in)}
              onChange={(checked) => toggle('mattermost_opt_in', checked)}
              disabled={!isStaff}
            />
            <ToggleRow
              label="Hide public achievement sharing"
              description="Keep achievements personal while still tracking them for yourself."
              checked={Boolean(user?.achievement_sharing_opt_out)}
              onChange={(checked) => toggle('achievement_sharing_opt_out', checked)}
            />
            {isStaff ? (
              <div className="border-t border-primary/10 pt-4">
                <p className="mb-2 block font-display font-black">Per-staff PayPal link</p>
                <p className="mb-3 text-sm font-bold text-berry-700 dark:text-rose-100">
                  Configure your personal PayPal direct-pay link. Any valid https PayPal link is allowed; leave empty to disable it.
                </p>
                <div className="flex gap-2">
                  <input
                    type="text"
                    value={paypalDraft}
                    onChange={(e) => setPaypalDraft(e.target.value)}
                    placeholder="https://www.paypal.com/paypalme/YourName"
                    className="min-h-11 w-full rounded-2xl border-2 border-primary/10 bg-[#fffdf8] px-3 font-bold text-berry-950 dark:bg-berry-950 dark:text-rose-50"
                  />
                  <Button
                    onClick={() => savePaypal()}
                    disabled={settings.isPending}
                  >
                    <Save className="size-4" />
                    Save
                  </Button>
                </div>
              </div>
            ) : null}
        </Card>

        <StockSettings canEdit={Boolean(user?.stock_manager)} />

        <Card>
            <p className="mb-3 font-display text-xl font-black">Session</p>
            <p className="mb-4 text-sm font-bold text-berry-700 dark:text-rose-100">
              Account deletion is not available in this backend. Logout only ends
              this browser session.
            </p>
            <Button variant="danger" onClick={logout}>
              <LogOut className="size-4" />
              Logout
            </Button>
        </Card>
      </div>
    </div>
  )
}

type StockDraft = Record<
  string,
  {
    current_amount: string
    critical_amount: string
    decrement_per_log: string
  }
>

function StockSettings({ canEdit }: { canEdit: boolean }) {
  const stock = useStock()
  const updateStock = useUpdateStockMutation()
  const { pushToast } = useToast()
  const [draft, setDraft] = useState<StockDraft>({})
  const [selectedStockId, setSelectedStockId] = useState('')

  useEffect(() => {
    if (!stock.data) return
    setDraft(
      Object.fromEntries(
        stock.data.items.map((item) => [
          item.id,
          {
            current_amount: String(item.current_amount),
            critical_amount: String(item.critical_amount),
            decrement_per_log: String(item.decrement_per_log),
          },
        ]),
      ),
    )
    setSelectedStockId((current) =>
      current && stock.data.items.some((item) => item.id === current)
        ? current
        : stock.data.items[0]?.id ?? '',
    )
  }, [stock.data])

  const setValue = (itemId: string, field: keyof StockDraft[string], value: string) => {
    setDraft((current) => ({
      ...current,
      [itemId]: {
        ...current[itemId],
        [field]: value,
      },
    }))
  }

  const save = (itemId: string) => {
    const values = draft[itemId]
    if (!values) return
    updateStock.mutate(
      {
        itemId,
        body: {
          current_amount: Number(values.current_amount),
          critical_amount: Number(values.critical_amount),
          decrement_per_log: Number(values.decrement_per_log),
        },
      },
      {
        onSuccess: () =>
          pushToast({
            tone: 'success',
            title: 'Stock saved',
            description: 'The shared stock settings are up to date.',
          }),
      },
    )
  }

  const selectedItem = stock.data?.items.find((item) => item.id === selectedStockId)
  const selectedValues = selectedItem ? draft[selectedItem.id] : undefined

  return (
    <Card>
      <div className="mb-4 flex items-center gap-3">
        <Package className="size-6 text-primary dark:text-blush" />
        <p className="font-display text-xl font-black">Stock</p>
      </div>
      <p className="mb-4 text-sm font-bold text-berry-700 dark:text-rose-100">
        {canEdit
          ? 'You can update current stock, critical thresholds, and per-drink usage.'
          : `Stock is shared. Only ${branding.staffLabel} can edit these values.`}
      </p>
      {stock.data?.items.length ? (
        <div className="space-y-3">
          <label className="block">
            <span className="mb-1 block text-xs font-black uppercase tracking-[0.14em] text-berry-700 dark:text-rose-100">
              Product
            </span>
            <select
              value={selectedStockId}
              onChange={(event) => setSelectedStockId(event.target.value)}
              className="min-h-11 w-full rounded-2xl border-2 border-primary/10 bg-[#fffdf8] px-3 font-bold text-berry-950 outline-none transition focus:border-primary dark:bg-berry-950 dark:text-rose-50"
            >
              {stock.data.items.map((item) => (
                <option key={item.id} value={item.id}>
                  {item.name}
                </option>
              ))}
            </select>
          </label>

          {selectedItem ? (
            <div className="rounded-3xl border border-berry-100 bg-white/70 p-4 text-berry-950 dark:border-white/10 dark:bg-white/10 dark:text-berry-50">
              <div className="mb-3 flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between">
                <p className="font-display text-lg font-black">{selectedItem.name}</p>
                <p className="text-sm font-black text-berry-700 dark:text-rose-100">
                  Current: {formatStockValue(selectedItem.current_amount, selectedItem.unit)}
                </p>
              </div>
              <div className="grid gap-3 sm:grid-cols-3">
                <StockNumberInput
                  label="Current"
                  value={selectedValues?.current_amount ?? ''}
                  disabled={!canEdit}
                  onChange={(value) => setValue(selectedItem.id, 'current_amount', value)}
                />
                <StockNumberInput
                  label="Critical"
                  value={selectedValues?.critical_amount ?? ''}
                  disabled={!canEdit}
                  onChange={(value) => setValue(selectedItem.id, 'critical_amount', value)}
                />
                <StockNumberInput
                  label="Per drink"
                  value={selectedValues?.decrement_per_log ?? ''}
                  disabled={!canEdit}
                  onChange={(value) => setValue(selectedItem.id, 'decrement_per_log', value)}
                />
              </div>
              {canEdit ? (
                <Button
                  className="mt-3"
                  onClick={() => save(selectedItem.id)}
                  disabled={updateStock.isPending}
                >
                  <Save className="size-4" />
                  Save {selectedItem.name}
                </Button>
              ) : null}
            </div>
          ) : null}
        </div>
      ) : (
        <p className="font-bold text-berry-700 dark:text-rose-100">No stock products configured.</p>
      )}
    </Card>
  )
}

function StockNumberInput({
  label,
  value,
  disabled,
  onChange,
}: {
  label: string
  value: string
  disabled: boolean
  onChange: (value: string) => void
}) {
  return (
    <label className="block">
      <span className="mb-1 block text-xs font-black uppercase tracking-[0.14em] text-berry-700 dark:text-rose-100">
        {label}
      </span>
      <input
        type="number"
        min="0"
        step="0.001"
        value={value}
        disabled={disabled}
        onChange={(event) => onChange(event.target.value)}
        className="min-h-11 w-full rounded-2xl border-2 border-primary/10 bg-[#fffdf8] px-3 font-bold text-berry-950 disabled:bg-berry-50 disabled:opacity-75 dark:bg-berry-950 dark:text-rose-50 dark:disabled:bg-berry-900"
      />
    </label>
  )
}

function ToggleRow({
  label,
  description,
  checked,
  onChange,
  disabled = false,
}: {
  label: string
  description: string
  checked: boolean
  onChange: (checked: boolean) => void
  disabled?: boolean
}) {
  return (
    <label
      className={`flex items-center justify-between gap-4 border-t border-primary/10 py-4 text-berry-950 first:border-t-0 dark:text-berry-50 ${disabled ? 'opacity-55' : ''
        }`}
    >
      <span className="min-w-0">
        <span className="block font-display font-black">{label}</span>
        <span className="text-sm font-bold text-berry-700 dark:text-rose-100">
          {description}
        </span>
      </span>
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(event) => onChange(event.target.checked)}
        className="size-4 shrink-0 accent-primary disabled:cursor-not-allowed sm:size-5"
      />
    </label>
  )
}

function ThemeButton({
  value,
  current,
  onSelect,
  icon,
  label,
}: {
  value: ThemeChoice
  current: ThemeChoice
  onSelect: (value: ThemeChoice) => void
  icon: ReactNode
  label: string
}) {
  return (
    <button
      type="button"
      onClick={() => onSelect(value)}
      className={`flex min-h-16 items-center justify-center gap-2 rounded-3xl border-2 px-4 font-display font-black transition ${current === value
        ? 'border-primary bg-primary text-white dark:border-blush dark:bg-blush dark:text-berry-950'
        : 'border-primary/10 bg-[#fffdf8] text-berry-950 hover:border-primary/30 dark:bg-white/10 dark:text-berry-50'
        }`}
    >
      {icon}
      {label}
    </button>
  )
}
