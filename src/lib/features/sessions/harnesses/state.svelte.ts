// Harness state module using Svelte 5 runes.
//
// The global listing is the fallback. Folder detections run inside the
// folder's Nix environment and are keyed by folder so out-of-order
// responses never clobber another folder.

import { backend } from '$lib/api/backend'
import { getExtensionCatalog, loadExtensionCatalog } from '$lib/features/extensions/catalog.svelte'
import { launchLabel } from '$lib/features/sessions/harnesses/catalog'
import { createFolderKeyedStore } from '$lib/state/folderKeyedStore.svelte'
import type { HarnessDescriptor, HarnessListing, HarnessStatus, SettingsChangedEvent } from '$lib/types/backend'

let listing = $state<HarnessListing>({ harnesses: [], default_preset: null })
let loading = $state(false)
const folderHarnesses = createFolderKeyedStore<HarnessListing>()

export function getHarnessesLoading() {
  return loading
}

export async function loadHarnesses() {
  loading = true
  try {
    listing = await backend.harnesses.list()
  } catch (e) {
    console.error('Failed to refresh harnesses:', e)
    throw e
  } finally {
    loading = false
  }
}

/** Detect harnesses inside the folder's environment. On failure the global listing applies. */
export async function loadHarnessesForFolder(folderPath: string, isActive: () => boolean = () => true) {
  void loadExtensionCatalog(folderPath).catch(() => {})
  const generation = folderHarnesses.generation(folderPath)
  loading = true
  try {
    const nextListing = await backend.harnesses.listForFolder(folderPath)
    if (!isActive() || folderHarnesses.generation(folderPath) !== generation) return
    folderHarnesses.set(folderPath, nextListing)
  } catch (e) {
    console.warn(`Failed to load harnesses for ${folderPath}:`, e)
  } finally {
    loading = false
  }
}

/** Global settings feed every loaded folder's effective harness configuration. */
export function refreshHarnessListings(event: SettingsChangedEvent): void {
  if (event.layer === 'global') {
    void loadHarnesses().catch(() => {})
    for (const folderPath of folderHarnesses.keys()) {
      void loadHarnessesForFolder(folderPath).catch(() => {})
    }
  } else if (event.folder_path && folderHarnesses.has(event.folder_path)) {
    void loadHarnessesForFolder(event.folder_path).catch(() => {})
  }
}

/** The folder's listing, falling back to global detection until loaded. */
export function getHarnessListing(folderPath: string | null): HarnessListing {
  return (folderPath ? folderHarnesses.get(folderPath) : undefined) ?? listing
}

/** Forget the folder's detection; called when the workbench releases the folder. */
export function releaseHarnessFolder(folderPath: string) {
  folderHarnesses.delete(folderPath)
}

export interface LaunchOption {
  harness: HarnessDescriptor
  preset: string | null
  status: HarnessStatus | null
  label: string
}

/** Enabled harness × (bare + presets) in catalog order; the resolved default preset first. */
export function getLaunchOptions(folderPath: string | null): LaunchOption[] {
  const listing = getHarnessListing(folderPath)
  const options: LaunchOption[] = []
  for (const harness of getExtensionCatalog(folderPath)?.runtime.harnesses ?? []) {
    const status = listing.harnesses.find((entry) => entry.id === harness.id) ?? null
    if (status?.enabled === false) continue
    options.push({ harness, preset: null, status, label: harness.label })
    for (const preset of status?.presets ?? []) {
      options.push({ harness, preset, status, label: launchLabel(harness.label, preset) })
    }
  }
  const defaultIndex = options.findIndex(
    ({ harness, preset }) => (preset === null ? harness.id : `${harness.id}/${preset}`) === listing.default_preset
  )
  if (defaultIndex > 0) options.unshift(options.splice(defaultIndex, 1)[0])
  return options
}

/** Parse `sessions.default_preset` against connected options; null when unset. */
export function resolveDefaultLaunch(folderPath: string | null): {
  option: LaunchOption | null
  configured: string | null
} {
  const configured = getHarnessListing(folderPath).default_preset
  if (configured === null) return { option: null, configured }
  const slash = configured.indexOf('/')
  const harnessId = slash < 0 ? configured : configured.slice(0, slash)
  const preset = slash < 0 ? null : configured.slice(slash + 1)
  const option =
    getLaunchOptions(folderPath).find(
      (option) => option.harness.id === harnessId && option.preset === preset && option.status?.status === 'connected'
    ) ?? null
  return { option, configured }
}
