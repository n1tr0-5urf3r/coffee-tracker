import { describe, expect, it } from 'vitest'
import { footerVersionLine } from './AppFooter'

describe('footerVersionLine', () => {
  it('renders the current tracker, frontend, and backend versions', () => {
    expect(footerVersionLine('1.6.2')).toBe(
      'Coffee Tracker v1.6.2 · Frontend v1.6.2 · Backend v1.6.2',
    )
  })
})
