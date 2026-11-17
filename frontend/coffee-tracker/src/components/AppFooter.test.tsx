import { describe, expect, it } from 'vitest'
import { footerVersionLine } from './AppFooter'

describe('footerVersionLine', () => {
  it('renders the current tracker, frontend, and backend versions', () => {
    expect(footerVersionLine('1.8.0')).toBe(
      'Coffee Tracker v1.8.0 · Frontend v1.8.0 · Backend v1.8.0',
    )
  })
})
