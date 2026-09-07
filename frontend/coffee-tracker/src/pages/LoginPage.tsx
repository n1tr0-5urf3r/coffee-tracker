import { useState, type FormEvent } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useLocation, useNavigate } from 'react-router-dom'
import { Coffee, HelpCircle, KeyRound, ShieldCheck } from 'lucide-react'
import { motion } from 'framer-motion'
import { api, isBackendUnavailableError } from '../api/client'
import { queryKeys } from '../api/queries'
import { Button } from '../components/Button'
import { Card } from '../components/Card'
import { LogoMark } from '../components/LogoMark'
import { BrandWatermark } from '../components/BrandWatermark'
import { branding } from '../config/branding'
import { useToast } from '../lib/toast'
import { getGuestRecoveryCodeForName, storeGuestRecoveryCode } from '../lib/guestRecovery'

export function LoginPage() {
  const [displayName, setDisplayName] = useState('')
  const [recoveryCode, setRecoveryCode] = useState('')
  const [guestLoginUsedCode, setGuestLoginUsedCode] = useState(false)
  const navigate = useNavigate()
  const location = useLocation()
  const queryClient = useQueryClient()
  const { pushToast } = useToast()

  const guestLogin = useMutation({
    mutationFn: api.guestLogin,
    onSuccess: (data) => {
      storeGuestRecoveryCode(data.guest_recovery_code, data.user.display_name)
      queryClient.setQueryData(queryKeys.me, data)
      pushToast({
        tone: 'success',
        title: guestLoginUsedCode ? 'Guest account restored' : 'Guest mug acquired',
        description: `Welcome, ${data.user.display_name}.`,
      })
      const from =
        (location.state as { from?: { pathname?: string } } | null)?.from
          ?.pathname ?? '/'
      navigate(from, { replace: true })
    },
    onError: (error) => {
      if (isBackendUnavailableError(error)) {
        pushToast({
          tone: 'error',
          title: 'Backend is currently unavailable',
          description: 'The coffee backend could not be reached. Please try again in a moment.',
        })
        return
      }
      if (error instanceof Error && error.message.includes('recovery_code')) {
        pushToast({
          tone: 'error',
          title: 'Guest code not found',
          description: 'Check the code from your other device, or leave the field empty.',
        })
        return
      }
      pushToast({
        tone: 'error',
        title: 'Name tag needs a tweak',
        description: 'Use at least two characters so everyone knows who tapped.',
      })
    },
  })

  const submitGuest = (event: FormEvent) => {
    event.preventDefault()
    const trimmedRecoveryCode = recoveryCode.trim()
    const savedRecoveryCode = getGuestRecoveryCodeForName(displayName)
    setGuestLoginUsedCode(Boolean(trimmedRecoveryCode || savedRecoveryCode))
    guestLogin.mutate({
      display_name: displayName,
      recovery_code:
        trimmedRecoveryCode === ''
          ? savedRecoveryCode || undefined
          : trimmedRecoveryCode,
    })
  }

  return (
    <main className="relative grid min-h-svh place-items-center overflow-hidden px-4 py-10">
      <div className="absolute inset-0 bg-[radial-gradient(circle_at_50%_10%,rgba(255,209,102,0.45),transparent_26rem)]" />
      <motion.div
        className="pointer-events-none absolute -right-10 top-8 w-44 rotate-12 opacity-20 sm:w-72 dark:opacity-25"
        animate={{ y: [0, -10, 0], rotate: [12, 8, 12] }}
        transition={{ duration: 6, repeat: Infinity, ease: 'easeInOut' }}
      >
        <BrandWatermark className="size-full" />
      </motion.div>
      <Card className="relative w-full max-w-lg p-7 sm:p-9">
        <div className="mb-8 flex justify-center">
          <LogoMark className="scale-125" />
        </div>
        <div className="text-center">
          <h1 className="font-display text-4xl font-black tracking-tight text-berry-950 sm:text-5xl dark:text-berry-50">
            Tap coffee.
            <span className="block text-primary dark:text-blush">
              Keep the streak.
            </span>
          </h1>
        </div>

        <div className="mt-8 grid gap-4">
          <a
            href={api.authentikLoginUrl}
            className="inline-flex min-h-14 items-center justify-center gap-3 rounded-3xl bg-berry-950 px-5 py-4 font-display font-black text-white shadow-button transition hover:-translate-y-0.5 hover:bg-berry-800 focus-visible:outline focus-visible:outline-4 focus-visible:outline-offset-4 focus-visible:outline-sun dark:bg-white dark:text-berry-950"
          >
            <ShieldCheck className="size-5" />
            {branding.oidcLoginLabel}
          </a>

          <form onSubmit={submitGuest} className="rounded-3xl bg-berry-100 p-3 dark:bg-white/10">
            <label
              htmlFor="guest-name"
              className="mb-2 block text-xs font-black text-berry-700 dark:text-rose-100"
            >
              Continue as guest
            </label>
            <div className="grid gap-2 sm:grid-cols-[1fr_auto]">
              <input
                id="guest-name"
                value={displayName}
                onChange={(event) => setDisplayName(event.target.value)}
                placeholder="Your display name"
                className="min-h-11 rounded-2xl border-2 border-primary/10 bg-white px-4 text-sm font-bold text-berry-950 placeholder:text-berry-700/45 dark:border-white/10 dark:bg-berry-950 dark:text-rose-50 dark:placeholder:text-rose-100/60 sm:col-span-2"
              />
              <div className="relative">
                <KeyRound
                  className="pointer-events-none absolute left-4 top-1/2 size-4 -translate-y-1/2 text-primary dark:text-blush"
                  aria-hidden="true"
                />
                <input
                  value={recoveryCode}
                  onChange={(event) => setRecoveryCode(event.target.value)}
                  placeholder="Guest code"
                  aria-describedby="guest-code-help"
                  className="min-h-11 w-full rounded-2xl border-2 border-primary/10 bg-white py-2.5 pl-11 pr-10 text-sm font-bold text-berry-950 placeholder:text-berry-700/45 dark:border-white/10 dark:bg-berry-950 dark:text-rose-50 dark:placeholder:text-rose-100/60"
                />
                <span className="group absolute right-3 top-1/2 inline-flex -translate-y-1/2">
                  <HelpCircle
                    className="size-4 text-berry-700 dark:text-rose-100"
                    aria-label="Guest code help"
                  />
                  <span
                    id="guest-code-help"
                    className="pointer-events-none absolute bottom-7 right-0 z-20 hidden w-64 rounded-2xl border border-white/10 bg-berry-950 px-3 py-2 text-xs font-bold leading-snug text-white shadow-soft group-hover:block group-focus-within:block dark:bg-berry-950 dark:text-berry-50"
                  >
                    Optional. Guests do not sign in with a provider, so this code links the same guest account across devices. Use it with the same guest name.
                  </span>
                </span>
              </div>
              <Button type="submit" disabled={guestLogin.isPending} className="min-h-11 px-4">
                <Coffee className="size-4" />
                Enter
              </Button>
            </div>
          </form>
        </div>
      </Card>
    </main>
  )
}
