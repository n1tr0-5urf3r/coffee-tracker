import { useEffect, useState } from 'react'
import {
  getQueuedLogs,
  removeQueuedLog,
  replayQueuedLogs,
  type QueuedLog,
} from '../lib/offlineQueue'
import { useQueryClient } from '@tanstack/react-query'
import { invalidateCoffeeData } from '../api/queries'
import { useToast } from '../lib/toast'

export function useOfflineQueue() {
  const [queuedLogs, setQueuedLogs] = useState<QueuedLog[]>([])
  const queryClient = useQueryClient()
  const { pushToast } = useToast()

  const refreshQueue = async () => setQueuedLogs(await getQueuedLogs())

  useEffect(() => {
    void refreshQueue()
    const replay = async () => {
      try {
        const sent = await replayQueuedLogs()
        if (sent.length > 0) {
          pushToast({
            tone: 'success',
            title: 'Kitchen WiFi caught up',
            description: `${sent.length} queued log${sent.length === 1 ? '' : 's'} replayed.`,
          })
          await invalidateCoffeeData(queryClient)
        }
      } catch {
        pushToast({
          tone: 'info',
          title: 'Still holding your queued taps',
          description: 'They will retry when the backend is reachable again.',
        })
      } finally {
        await refreshQueue()
      }
    }
    window.addEventListener('online', replay)
    return () => window.removeEventListener('online', replay)
  }, [pushToast, queryClient])

  return {
    queuedLogs,
    refreshQueue,
    cancelQueuedLog: async (id: string) => {
      await removeQueuedLog(id)
      await refreshQueue()
    },
  }
}
