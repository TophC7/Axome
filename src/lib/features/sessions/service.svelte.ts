import { getHarness, launchLabel } from '$lib/features/sessions/harnesses/catalog'
import { getHarnessListing, getLaunchOptions } from '$lib/features/sessions/harnesses/state.svelte'
import { notify } from '$lib/features/notifications/state.svelte'
import { isProcessLive, type SessionTab, type TabId } from '$lib/features/workbench/model'
import { addSessionTab, getTabs, setActiveTab } from '$lib/features/workbench/state.svelte'
import type { IndexedSession } from '$lib/types/backend'
import { getSessionIndex } from './state.svelte'

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

export function findIndexedSessionTab(folderPath: string, session: IndexedSession): SessionTab | undefined {
  return getTabs().find(
    (tab): tab is SessionTab =>
      tab.kind === 'session' &&
      tab.folderPath === folderPath &&
      tab.harnessId === session.harnessId &&
      tab.sessionId === session.sessionId
  )
}

export function historyRows(folderPath: string): IndexedSession[] {
  return (getSessionIndex(folderPath).snapshot?.sessions ?? []).filter(
    (session) => !findIndexedSessionTab(folderPath, session)
  )
}

export function sessionLaunchPreset(folderPath: string, session: IndexedSession): string | null {
  const configured = getHarnessListing(folderPath).default_preset
  if (configured !== null) {
    const slash = configured.indexOf('/')
    const harnessId = slash < 0 ? configured : configured.slice(0, slash)
    const preset = slash < 0 ? null : configured.slice(slash + 1)
    if (harnessId === session.harnessId && session.presets.includes(preset)) return preset
  }
  return session.presets[0]
}

export function openIndexedSession(folderPath: string, session: IndexedSession, preset: string | null): void {
  const tab = findIndexedSessionTab(folderPath, session)
  if (tab) {
    setActiveTab(tab.id)
    return
  }
  const option = getLaunchOptions(folderPath).find(
    (option) => option.harness.id === session.harnessId && option.preset === preset
  )
  if (option?.status?.status !== 'connected') {
    const label = launchLabel(getHarness(folderPath, session.harnessId)?.label ?? session.harnessId, preset)
    notify.error(`${label} unavailable`, `Install or enable ${label} in Settings → Harnesses first.`)
    return
  }
  addSessionTab(folderPath, {
    harnessId: session.harnessId,
    preset,
    title: session.title ?? 'Untitled Session',
    sessionId: session.sessionId
  })
}
