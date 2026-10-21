import { backend } from '$lib/api/backend'
import { launchLabel } from '$lib/features/sessions/harnesses/catalog'
import { createFolderKeyedStore } from '$lib/state/folderKeyedStore.svelte'
import type { ShellListing, SettingsChangedEvent } from '$lib/types/backend'

let listing = $state<ShellListing>({ shell: '', presets: [], default_preset: null })
let loading = $state(false)
const folderShells = createFolderKeyedStore<ShellListing>()

export function getShellsLoading() {
  return loading
}

export async function loadShells() {
  loading = true
  try {
    listing = await backend.shells.list()
  } catch (error) {
    console.error('Failed to refresh shells:', error)
    throw error
  } finally {
    loading = false
  }
}

/** Folder detection uses its environment; the global listing remains the fallback. */
export async function loadShellsForFolder(folderPath: string, isActive: () => boolean = () => true) {
  const generation = folderShells.generation(folderPath)
  loading = true
  try {
    const nextListing = await backend.shells.listForFolder(folderPath)
    if (!isActive() || folderShells.generation(folderPath) !== generation) return
    folderShells.set(folderPath, nextListing)
  } catch (error) {
    console.warn(`Failed to load shells for ${folderPath}:`, error)
  } finally {
    loading = false
  }
}

export async function reloadShellListings(): Promise<void> {
  await Promise.all([loadShells(), ...Array.from(folderShells.keys(), (folderPath) => loadShellsForFolder(folderPath))])
}

export function refreshShellListings(event: SettingsChangedEvent): void {
  if (event.layer === 'global') {
    void reloadShellListings().catch(() => {})
  } else if (event.folder_path && folderShells.has(event.folder_path)) {
    void loadShellsForFolder(event.folder_path)
  }
}

export function getShellListing(folderPath: string | null): ShellListing {
  return (folderPath ? folderShells.get(folderPath) : undefined) ?? listing
}

export function releaseShellFolder(folderPath: string) {
  folderShells.delete(folderPath)
}

export interface ShellLaunchOption {
  preset: string | null
  label: string
  shell: string
}

export function getShellLaunchOptions(folderPath: string | null): ShellLaunchOption[] {
  const listing = getShellListing(folderPath)
  const options: ShellLaunchOption[] = [
    { preset: null, label: launchLabel('Terminal', null), shell: listing.shell },
    ...listing.presets.map(({ name, shell }) => ({ preset: name, label: launchLabel('Terminal', name), shell }))
  ]
  const defaultIndex = options.findIndex(({ preset }) => preset === listing.default_preset)
  if (defaultIndex > 0) options.unshift(options.splice(defaultIndex, 1)[0])
  return options
}
