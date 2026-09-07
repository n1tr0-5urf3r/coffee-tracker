const CACHE_NAME = 'coffee-tracker-shell-v2'
const QUEUE_DB = 'coffee-tracker-queue'
const QUEUE_STORE = 'logs'

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(CACHE_NAME).then((cache) =>
      cache.addAll([
        '/',
        '/config.js',
        '/favicon.svg',
        '/manifest.webmanifest',
      ]),
    ),
  )
  self.skipWaiting()
})

self.addEventListener('activate', (event) => {
  event.waitUntil(self.clients.claim())
})

self.addEventListener('fetch', (event) => {
  const url = new URL(event.request.url)
  if (
    event.request.method === 'POST' &&
    url.pathname === '/api/v1/consumptions'
  ) {
    event.respondWith(
      fetch(event.request.clone()).catch(async () => {
        const payload = await event.request.clone().json()
        await putQueuedLog({
          id: crypto.randomUUID(),
          machine_id: payload.machine_id,
          beverage_id: payload.beverage_id,
          created_at: Date.now(),
        })
        return new Response(
          JSON.stringify({
            id: `queued-${Date.now()}`,
            user_id: 'offline',
            machine_id: payload.machine_id,
            beverage_id: payload.beverage_id,
            consumed_at: Math.floor(Date.now() / 1000),
          }),
          {
            status: 202,
            headers: { 'content-type': 'application/json' },
          },
        )
      }),
    )
    return
  }

  if (event.request.method === 'GET' && url.origin === location.origin) {
    event.respondWith(
      fetch(event.request).catch(() => caches.match(event.request)),
    )
  }
})

self.addEventListener('message', (event) => {
  if (event.data?.type === 'COFFEE_LOG_QUEUED') {
    void replayQueue()
  }
})

self.addEventListener('sync', (event) => {
  if (event.tag === 'coffee-log-sync') {
    event.waitUntil(replayQueue())
  }
})

function openQueueDb() {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(QUEUE_DB, 1)
    request.onupgradeneeded = () => {
      request.result.createObjectStore(QUEUE_STORE, { keyPath: 'id' })
    }
    request.onsuccess = () => resolve(request.result)
    request.onerror = () => reject(request.error)
  })
}

async function putQueuedLog(log) {
  const db = await openQueueDb()
  return new Promise((resolve, reject) => {
    const tx = db.transaction(QUEUE_STORE, 'readwrite')
    tx.objectStore(QUEUE_STORE).put(log)
    tx.oncomplete = () => resolve()
    tx.onerror = () => reject(tx.error)
  })
}

async function getQueuedLogs() {
  const db = await openQueueDb()
  return new Promise((resolve, reject) => {
    const tx = db.transaction(QUEUE_STORE, 'readonly')
    const request = tx.objectStore(QUEUE_STORE).getAll()
    request.onsuccess = () => resolve(request.result)
    request.onerror = () => reject(request.error)
  })
}

async function deleteQueuedLog(id) {
  const db = await openQueueDb()
  return new Promise((resolve, reject) => {
    const tx = db.transaction(QUEUE_STORE, 'readwrite')
    tx.objectStore(QUEUE_STORE).delete(id)
    tx.oncomplete = () => resolve()
    tx.onerror = () => reject(tx.error)
  })
}

async function replayQueue() {
  const logs = await getQueuedLogs()
  for (const log of logs) {
    const response = await fetch('/api/v1/consumptions', {
      method: 'POST',
      credentials: 'include',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        machine_id: log.machine_id,
        beverage_id: log.beverage_id,
      }),
    })
    if (response.ok) {
      await deleteQueuedLog(log.id)
    }
    if (response.status === 401) {
      return
    }
  }
}
