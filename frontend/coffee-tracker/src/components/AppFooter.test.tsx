import { describe, expect, it } from 'vitest'
import { footerVersionLine } from './AppFooter'

describe('footerVersionLine', () => {
  it('renders the current tracker, frontend, and backend versions', () => {
    expect(footerVersionLine('1.7.0')).toBe(
      'Coffee Tracker v1.7.0 · Frontend v1.7.0 · Backend v1.7.0',
    )
  })
})
