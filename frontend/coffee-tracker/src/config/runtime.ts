export type RuntimeConfig = {
  API_BASE_URL?: string
  APP_NAME?: string
  ORGANIZATION_NAME?: string
  ORGANIZATION_SHORT_NAME?: string
  STAFF_LABEL?: string
  OIDC_LOGIN_LABEL?: string
  LOGO_URL?: string
}

declare global {
  interface Window {
    __COFFEE_TRACKER_CONFIG__?: RuntimeConfig
  }
}

export const runtimeConfig = window.__COFFEE_TRACKER_CONFIG__ ?? {}
