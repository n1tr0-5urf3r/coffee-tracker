import { afterEach, describe, expect, it, vi } from 'vitest'
import { ApiError, api, isBackendUnavailableError } from './client'

describe('isBackendUnavailableError', () => {
  it('recognizes network and upstream gateway failures', () => {
    expect(isBackendUnavailableError(new ApiError(0, 'network', null))).toBe(true)
    expect(isBackendUnavailableError(new ApiError(500, 'proxy failed', null))).toBe(true)
    expect(isBackendUnavailableError(new ApiError(502, 'bad gateway', null))).toBe(true)
    expect(isBackendUnavailableError(new ApiError(503, 'unavailable', null))).toBe(true)
    expect(isBackendUnavailableError(new ApiError(504, 'timeout', null))).toBe(true)
  })

  it('recognizes Vite proxy failure messages without relying on ApiError identity', () => {
    expect(isBackendUnavailableError({ status: 500 })).toBe(true)
    expect(isBackendUnavailableError(new Error('connect ECONNREFUSED 127.0.0.1:3124'))).toBe(
      true,
    )
  })

  it('does not classify validation errors as backend outages', () => {
    expect(isBackendUnavailableError(new ApiError(400, 'bad request', null))).toBe(false)
  })
})

describe('request error parsing', () => {
  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('throws ApiError when the proxy returns a non-JSON 502 body', async () => {
    const fetchMock = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValue(
        new Response('Bad Gateway', {
          status: 502,
          headers: { 'content-type': 'text/plain' },
        }),
      )

    await expect(api.version()).rejects.toMatchObject({
      name: 'ApiError',
      status: 502,
    })
    expect(fetchMock).toHaveBeenCalledTimes(1)
  })

  it('sends the selected staff recipient when marking a payment paid', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      Response.json({
        id: 'payment',
        user_id: 'target',
        received_by_user_id: 'actor',
        amount_cents: 500,
        drink_count: 10,
        period_end_ts: 1,
        paid_at: 1,
      }),
    )

    await api.markPaid({
      user_id: 'target',
      received_by_user_id: 'actor',
      note: 'Marked paid from test',
    })

    expect(fetchMock).toHaveBeenCalledWith(
      '/api/v1/payments',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({
          user_id: 'target',
          received_by_user_id: 'actor',
          note: 'Marked paid from test',
        }),
      }),
    )
  })
})
