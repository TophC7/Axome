// Folder-keyed harness session history cached on the folder's owning host.
//
// Activation reads the cache, then asks the host to refresh it; the host
// announces completed refreshes through one shared `sessions-changed`
// subscription, which only ever re-reads the cache.

import { untrack } from 'svelte'
import { backend } from '$lib/api/backend'
import type { Unsubscribe } from '$lib/api/transport'
import { createFolderKeyedStore } from '$lib/state/folderKeyedStore.svelte'
import type { SessionIndexSnapshot } from '$lib/types/backend'
import { getErrorMessage } from '$lib/utils/client-error'

interface SessionIndexEntry {
  snapshot: SessionIndexSnapshot | null
  loading: boolean
  error: string | null
}

const EMPTY: SessionIndexEntry = { snapshot: null, loading: false, error: null }

const store = createFolderKeyedStore<SessionIndexEntry>()
const requests = new Map<string, number>()
// Folders with an event-driven re-read in flight; `true` once a trailing read is owed.
const rereads = new Map<string, boolean>()

let subscribers = 0
let subscription: Promise<Unsubscribe> | null = null

export function getSessionIndex(folderPath: string): SessionIndexEntry {
  return store.get(folderPath) ?? EMPTY
}

/** Latest request wins; release (store generation) fences everything in flight. */
async function load(folderPath: string, refresh: boolean): Promise<void> {
  const generation = store.generation(folderPath)
  const request = (requests.get(folderPath) ?? 0) + 1
  requests.set(folderPath, request)
  const current = () => store.generation(folderPath) === generation && requests.get(folderPath) === request
  // Callers may run inside an effect; reading the store there would re-trigger it.
  untrack(() => store.set(folderPath, { ...getSessionIndex(folderPath), loading: true }))
  try {
    const snapshot = await (refresh ? backend.sessionIndex.refresh(folderPath) : backend.sessionIndex.get(folderPath))
    if (current()) store.set(folderPath, { snapshot, loading: false, error: null })
  } catch (error) {
    if (current()) store.patch(folderPath, { loading: false, error: getErrorMessage(error) })
  }
}

/** One refresh can announce many changes: keep one re-read in flight plus one trailing read. */
function reread(folderPath: string): void {
  if (rereads.has(folderPath)) {
    rereads.set(folderPath, true)
    return
  }
  rereads.set(folderPath, false)
  void load(folderPath, false).finally(() => {
    const trailing = rereads.get(folderPath)
    rereads.delete(folderPath)
    if (trailing && store.has(folderPath)) reread(folderPath)
  })
}

function retain(): void {
  if (subscribers++ > 0) return
  subscription = backend.sessionIndex.onChanged(({ folderPath }) => {
    if (store.has(folderPath)) reread(folderPath)
  })
}

function unretain(): void {
  if (--subscribers > 0 || !subscription) return
  const pending = subscription
  subscription = null
  void pending.then((unsubscribe) => unsubscribe())
}

/** Load the cache, request a host refresh and follow change events while the folder is shown. */
export function activateSessionsFolder(folderPath: string): () => void {
  retain()
  void load(folderPath, false).then(() => {
    if (store.has(folderPath)) void load(folderPath, true)
  })
  let released = false
  return () => {
    if (released) return
    released = true
    unretain()
  }
}

/** Explicit refresh; failures land in the entry's `error`. */
export function refreshSessionIndex(folderPath: string): Promise<void> {
  return load(folderPath, true)
}

/** Refresh every loaded folder `affects` selects (settings, Nix and environment changes). */
export function refreshLoadedSessionIndexes(affects: (folderPath: string) => boolean): void {
  for (const folderPath of [...store.keys()]) {
    if (affects(folderPath)) void load(folderPath, true)
  }
}

/** Forget the folder's history; called when the workbench releases the folder. */
export function releaseSessionsFolder(folderPath: string): void {
  store.delete(folderPath)
  requests.delete(folderPath)
}
