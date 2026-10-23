import { backend } from '$lib/api/backend'
import type { ChatModelCatalog } from '$lib/types/backend'
import { getErrorMessage } from '$lib/utils/client-error'
import { splitRemotePath } from '$lib/utils/paths'

type ModelState = { catalog: ChatModelCatalog | null; loading: boolean; error: string | null }

const hosts = new Map<string, ModelState>()
const loads = new Map<string, Promise<void>>()

export function getChatModels(folderPath: string): ModelState {
  const key = splitRemotePath(folderPath)?.server ?? ''
  let host = hosts.get(key)
  if (!host) {
    const state = $state<ModelState>({ catalog: null, loading: false, error: null })
    host = state
    hosts.set(key, host)
  }
  return host
}

export function loadChatModels(folderPath: string, force = false): Promise<void> {
  const key = splitRemotePath(folderPath)?.server ?? ''
  const existing = loads.get(key)
  if (existing) return existing
  const host = getChatModels(folderPath)
  if (host.catalog && !force) return Promise.resolve()

  host.loading = true
  host.error = null
  const load = backend.chat
    .models(folderPath)
    .then((catalog) => {
      host.catalog = catalog
    })
    .catch((error: unknown) => {
      host.error = getErrorMessage(error)
    })
    .finally(() => {
      host.loading = false
      loads.delete(key)
    })
  loads.set(key, load)
  return load
}
