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

