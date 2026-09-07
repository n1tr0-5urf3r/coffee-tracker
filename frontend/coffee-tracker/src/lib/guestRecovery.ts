const guestRecoveryCodeKey = 'coffee_guest_recovery_code'
const guestRecoveryCodesKey = 'coffee_guest_recovery_codes_by_name'

export function getGuestRecoveryCode(displayName?: string | null) {
  if (displayName) {
    const code = getGuestRecoveryCodeForName(displayName)
    if (code) return code
  }
  return window.localStorage.getItem(guestRecoveryCodeKey) ?? ''
}

export function getGuestRecoveryCodeForName(displayName: string) {
  const codes = getGuestRecoveryCodes()
  return codes[guestRecoveryNameKey(displayName)] ?? ''
}

export function storeGuestRecoveryCode(code?: string | null, displayName?: string | null) {
  if (!code) return
  window.localStorage.setItem(guestRecoveryCodeKey, code)
  if (!displayName) return
  const codes = getGuestRecoveryCodes()
  codes[guestRecoveryNameKey(displayName)] = code
  window.localStorage.setItem(guestRecoveryCodesKey, JSON.stringify(codes))
}

export function formatGuestRecoveryCode(code: string) {
  return code.replace(/(.{4})(?=.)/g, '$1-')
}

function getGuestRecoveryCodes() {
  const stored = window.localStorage.getItem(guestRecoveryCodesKey)
  if (!stored) return {}
  try {
    const parsed = JSON.parse(stored)
    if (parsed && typeof parsed === 'object') {
      return parsed as Record<string, string>
    }
  } catch {
    // Ignore invalid old/local edits and rebuild as codes are generated.
  }
  return {}
}

function guestRecoveryNameKey(displayName: string) {
  return displayName.trim().toLocaleLowerCase()
}
