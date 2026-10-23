import { getHarness, launchLabel } from '$lib/features/sessions/harnesses/catalog'
import { getHarnessListing, getLaunchOptions } from '$lib/features/sessions/harnesses/state.svelte'
import { notify } from '$lib/features/notifications/state.svelte'
import { isProcessLive, type ChatTab, type SessionTab, type TabId } from '$lib/features/workbench/model'
import { addChatTab, addSessionTab, getTabs, setActiveTab } from '$lib/features/workbench/state.svelte'
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
  return getTabs().filter(
    (tab) =>
      tab.folderPath === folderPath &&
      ((tab.kind === 'session' && isProcessLive(tab.status)) || (tab.kind === 'chat' && tab.status === 'running'))
  ).length
}

export function findIndexedSessionTab(folderPath: string, session: IndexedSession): SessionTab | ChatTab | undefined {
  const source = session.source
  return getTabs().find(
    (tab): tab is SessionTab | ChatTab =>
      tab.folderPath === folderPath &&
      (tab.kind === 'session' || tab.kind === 'chat') &&
      tab.sessionId === session.sessionId &&
      (source.kind === 'axo' ? tab.kind === 'chat' : tab.kind === 'session' && tab.harnessId === source.harnessId)
  )
}

export function historyRows(folderPath: string): IndexedSession[] {
  return (getSessionIndex(folderPath).snapshot?.sessions ?? []).filter(
    (session) => !findIndexedSessionTab(folderPath, session)
  )
}

export function sessionLaunchPreset(folderPath: string, session: IndexedSession): string | null {
  const source = session.source
  if (source.kind !== 'harness') return null
  const configured = getHarnessListing(folderPath).default_preset
  if (configured !== null) {
    const slash = configured.indexOf('/')
    const harnessId = slash < 0 ? configured : configured.slice(0, slash)
    const preset = slash < 0 ? null : configured.slice(slash + 1)
    if (harnessId === source.harnessId && source.presets.includes(preset)) return preset
  }
  return source.presets[0]
}

export function openIndexedSession(folderPath: string, session: IndexedSession, preset: string | null): void {
  const source = session.source
  if (source.kind === 'axo') {
    openChatSession(folderPath, session.sessionId, session.title ?? 'Untitled Chat')
    return
  }
  const tab = findIndexedSessionTab(folderPath, session)
  if (tab) {
    setActiveTab(tab.id)
    return
  }
  const option = getLaunchOptions(folderPath).find(
    (option) => option.harness.id === source.harnessId && option.preset === preset
  )
  if (option?.status?.status !== 'connected') {
    const label = launchLabel(getHarness(folderPath, source.harnessId)?.label ?? source.harnessId, preset)
    notify.error(`${label} unavailable`, `Install or enable ${label} in Settings → Harnesses first.`)
    return
  }
  addSessionTab(folderPath, {
    harnessId: source.harnessId,
    preset,
    title: session.title ?? 'Untitled Session',
    sessionId: session.sessionId
  })
}

export function newChat(folderPath: string): TabId {
  const tabId = addChatTab(folderPath, { sessionId: null, title: 'New Chat' })
  setActiveTab(tabId)
  return tabId
}

export function openChatSession(folderPath: string, sessionId: string, title: string): TabId {
  const existing = getTabs().find(
    (tab) => tab.kind === 'chat' && tab.folderPath === folderPath && tab.sessionId === sessionId
  )
  const tabId = existing?.id ?? addChatTab(folderPath, { sessionId, title })
  setActiveTab(tabId)
  return tabId
}
