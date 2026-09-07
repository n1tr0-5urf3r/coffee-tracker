import { openDB, type DBSchema } from 'idb'
import { api, type ConsumptionResponse } from '../api/client'

export type QueuedLog = {
  id: string
  machine_id: string
  beverage_id: string
  created_at: number
}

interface CoffeeQueueDb extends DBSchema {
  logs: {
    key: string
    value: QueuedLog
  }
}

const dbPromise = openDB<CoffeeQueueDb>('coffee-tracker-queue', 1, {
  upgrade(db) {
    db.createObjectStore('logs', { keyPath: 'id' })
  },
})

export async function queueLog(input: Omit<QueuedLog, 'id' | 'created_at'>) {
  const item: QueuedLog = {
    ...input,
    id: crypto.randomUUID(),
    created_at: Date.now(),
  }
  const db = await dbPromise
  await db.put('logs', item)
  await navigator.serviceWorker?.ready.then((registration) => {
    registration.active?.postMessage({ type: 'COFFEE_LOG_QUEUED' })
  })
  return item
}

export async function getQueuedLogs() {
  const db = await dbPromise
  return db.getAll('logs')
}

export async function removeQueuedLog(id: string) {
  const db = await dbPromise
  await db.delete('logs', id)
}

export async function replayQueuedLogs() {
  const logs = await getQueuedLogs()
  const sent: ConsumptionResponse[] = []
  for (const log of logs) {
    const response = await api.createConsumption({
      machine_id: log.machine_id,
      beverage_id: log.beverage_id,
    })
    await removeQueuedLog(log.id)
    sent.push(response)
  }
  return sent
}
