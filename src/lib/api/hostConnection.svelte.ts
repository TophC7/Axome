// The web page's control connection to its server; null on desktop.

import type { WebConnectionState } from '$lib/api/transport.web'

let state = $state<WebConnectionState | null>(null)

export function getHostConnection(): WebConnectionState | null {
  return state
}

export function setHostConnection(next: WebConnectionState): void {
  state = next
}
