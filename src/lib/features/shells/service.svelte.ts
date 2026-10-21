import { launchLabel } from '$lib/features/sessions/harnesses/catalog'
import type { TabId } from '$lib/features/workbench/model'
import { addShellTab } from '$lib/features/workbench/state.svelte'

export function startShell(folderPath: string, preset: string | null): TabId {
  return addShellTab(folderPath, { preset, title: launchLabel('Terminal', preset) })
}
