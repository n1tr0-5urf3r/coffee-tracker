const germanDateFormatter = new Intl.DateTimeFormat('de-DE', {
  day: '2-digit',
  month: '2-digit',
  year: 'numeric',
})

export function formatDate(value: Date | number | string) {
  return germanDateFormatter.format(value instanceof Date ? value : new Date(value))
}

export function formatUnixDate(
  timestampSeconds: number | null | undefined,
  fallback = 'n/a',
) {
  if (!timestampSeconds) {
    return fallback
  }

  return formatDate(timestampSeconds * 1000)
}
