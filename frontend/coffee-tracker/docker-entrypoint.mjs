import { spawn } from 'node:child_process'
import { writeFileSync } from 'node:fs'

const keys = [
  'API_BASE_URL',
  'APP_NAME',
  'ORGANIZATION_NAME',
  'ORGANIZATION_SHORT_NAME',
  'STAFF_LABEL',
  'OIDC_LOGIN_LABEL',
  'LOGO_URL',
]
const config = Object.fromEntries(
  keys.flatMap((key) => {
    const value = process.env[key]?.trim()
    return value ? [[key, value]] : []
  }),
)

writeFileSync(
  '/app/dist/config.js',
  `window.__COFFEE_TRACKER_CONFIG__ = ${JSON.stringify(config)};\n`,
)

const child = spawn(
  'npm',
  ['run', 'preview', '--', '--host', '0.0.0.0', '--port', '3123'],
  { stdio: 'inherit' },
)

for (const signal of ['SIGINT', 'SIGTERM']) {
  process.on(signal, () => child.kill(signal))
}
child.on('exit', (code, signal) => {
  if (signal) process.kill(process.pid, signal)
  else process.exit(code ?? 1)
})
