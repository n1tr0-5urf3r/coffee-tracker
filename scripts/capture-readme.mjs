import { execFileSync } from 'node:child_process'
import { createHash, randomBytes } from 'node:crypto'
import { mkdirSync, writeFileSync } from 'node:fs'

const database = '/tmp/coffee-tracker-screenshots.db'
const outputDirectory = new URL('../docs/screenshots/', import.meta.url)
const sessionSecret = 'screenshot-session-secret'
const sessionToken = randomBytes(32).toString('base64url')
const sessionHash = createHash('sha256')
  .update(`${sessionSecret}:${sessionToken}`)
  .digest('base64url')
const now = Math.floor(Date.now() / 1000)

execFileSync('sqlite3', [
  database,
  `UPDATE users SET display_name = 'Member ' || printf('%02d', rowid), email = NULL, paypal_link = CASE WHEN paypal_link IS NULL THEN NULL ELSE 'https://paypal.me/coffee-demo' END;
   UPDATE machines SET name = 'Coffee machine', location = 'Common area';
   UPDATE expense_entries SET description = 'Shared supplies';
   UPDATE payments SET note = 'Demo payment';`,
])
const userId = execFileSync('sqlite3', [
  database,
  `SELECT u.id FROM users u
   LEFT JOIN consumptions c ON c.user_id = u.id
   WHERE u.stock_manager = 1
   GROUP BY u.id
   ORDER BY COUNT(c.id) DESC
   LIMIT 1;`,
], { encoding: 'utf8' }).trim()

if (!userId) throw new Error('No staff user is available in the screenshot database')
execFileSync('sqlite3', [
  database,
  `INSERT INTO sessions (id_hash, user_id, expires_at, revoked_at, created_at)
   VALUES ('${sessionHash}', '${userId}', ${now + 3600}, NULL, ${now});`,
])

const page = await fetch(
  'http://127.0.0.1:9222/json/new?http%3A%2F%2Flocalhost%3A3123%2Flogin',
  { method: 'PUT' },
).then((response) => response.json())
const socket = new WebSocket(page.webSocketDebuggerUrl)
await new Promise((resolve, reject) => {
  socket.addEventListener('open', resolve, { once: true })
  socket.addEventListener('error', reject, { once: true })
})

let commandId = 0
const pending = new Map()
socket.addEventListener('message', (event) => {
  const message = JSON.parse(event.data)
  if (!message.id) return
  const callback = pending.get(message.id)
  if (!callback) return
  pending.delete(message.id)
  if (message.error) callback.reject(new Error(message.error.message))
  else callback.resolve(message.result)
})

function command(method, params = {}) {
  const id = ++commandId
  socket.send(JSON.stringify({ id, method, params }))
  return new Promise((resolve, reject) => pending.set(id, { resolve, reject }))
}

const pause = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds))

await command('Page.enable')
await command('Network.enable')
await command('Emulation.setDeviceMetricsOverride', {
  width: 1440,
  height: 1000,
  deviceScaleFactor: 1,
  mobile: false,
})
await pause(1800)
mkdirSync(outputDirectory, { recursive: true })

async function capture(name) {
  await command('Runtime.evaluate', {
    expression: `(() => {
      const style = document.createElement('style');
      style.textContent = '*,*::before,*::after{animation:none!important;transition:none!important}';
      document.head.appendChild(style);
      window.scrollTo(0, 0);
    })()`,
  })
  await pause(500)
  const { data } = await command('Page.captureScreenshot', {
    format: 'png',
    fromSurface: true,
    captureBeyondViewport: false,
  })
  writeFileSync(new URL(`${name}.png`, outputDirectory), Buffer.from(data, 'base64'))
}

async function navigate(path) {
  await command('Page.navigate', { url: `http://localhost:3123${path}` })
  await pause(2200)
}

await capture('login')
await command('Network.setCookie', {
  name: 'coffee_session',
  value: sessionToken,
  domain: 'localhost',
  path: '/',
  httpOnly: true,
  sameSite: 'Lax',
})

for (const [name, path] of [
  ['log', '/'],
  ['dashboard', '/dashboard'],
  ['achievements', '/achievements'],
  ['leaderboard', '/leaderboard'],
  ['payments', '/payments'],
  ['settings', '/settings'],
]) {
  await navigate(path)
  await capture(name)
}

socket.close()
