import { runtimeConfig } from './runtime'

function envValue(value: string | undefined, fallback: string) {
  const trimmed = value?.trim()
  return trimmed ? trimmed : fallback
}

export const branding = {
  appName: envValue(runtimeConfig.APP_NAME, 'Coffee Tracker'),
  organizationName: envValue(
    runtimeConfig.ORGANIZATION_NAME,
    'the community',
  ),
  organizationShortName: envValue(
    runtimeConfig.ORGANIZATION_SHORT_NAME,
    'Community',
  ),
  staffLabel: envValue(runtimeConfig.STAFF_LABEL, 'staff'),
  oidcLoginLabel: envValue(
    runtimeConfig.OIDC_LOGIN_LABEL,
    'Sign in with OpenID Connect',
  ),
  logoUrl: runtimeConfig.LOGO_URL?.trim() || null,
  githubUrl: 'https://github.com/n1tr0-5urf3r/coffee-tracker',
}
