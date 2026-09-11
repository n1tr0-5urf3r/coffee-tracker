/** Calendar-month helpers for the Stempelkarte, working on `YYYY-MM` keys. */

export function monthKey(date: Date) {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}`
}

export function currentMonthKey() {
  return monthKey(new Date())
}

export function shiftMonth(key: string, delta: number) {
  const [year, month] = key.split('-').map(Number)
  const date = new Date(year, month - 1 + delta, 1)
  return monthKey(date)
}

export function formatMonthLabel(key: string, locale = 'en-GB') {
  const [year, month] = key.split('-').map(Number)
  return new Date(year, month - 1, 1).toLocaleDateString(locale, {
    month: 'long',
    year: 'numeric',
  })
}
