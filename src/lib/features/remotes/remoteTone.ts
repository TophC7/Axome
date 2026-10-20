import type { StatusTone } from '$lib/components/ui/status-dot'
import type { RemoteStatus } from '$lib/types/backend'

export function remoteTone(state: RemoteStatus['state'] | 'checking'): StatusTone {
  switch (state) {
    case 'connected':
      return 'success'
    case 'error':
      return 'danger'
    case 'reconnecting':
      return 'waiting'
    default:
      return 'idle'
  }
}
