import { getHarness, launchLabel } from '$lib/features/sessions/harnesses/catalog'
import { isProcessLive, type TabId } from '$lib/features/workbench/model'
import { addSessionTab, getTabs } from '$lib/features/workbench/state.svelte'

/** Open a dormant tab; the terminal surface starts it on activation. */
export function startSession(folderPath: string, harnessId: string, preset: string | null): TabId {
  const title =
    launchLabel(getHarness(folderPath, harnessId)?.label ?? harnessId, preset) + (preset === null ? ' session' : '')
  return addSessionTab(folderPath, { harnessId, preset, title, sessionId: null })
}

/** Live agents above one share the folder's working tree. */
export function countLiveAgents(folderPath: string): number {
  return getTabs().filter((tab) => tab.kind === 'session' && tab.folderPath === folderPath && isProcessLive(tab.status))
    .length
}
