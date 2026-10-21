import { backend } from '$lib/api/backend'
import { notify } from '$lib/features/notifications/state.svelte'
import * as sessionRegistry from '$lib/features/sessions/terminal/sessionRegistry'
import type { TerminalSessionManager } from '$lib/features/sessions/terminal/TerminalSessionManager'
import { getHarness, launchLabel } from '$lib/features/sessions/harnesses/catalog'
import { isProcessLive, type SessionTab, type TabId } from '$lib/features/workbench/model'
import {
  addSessionTab,
  clearSessionTabSessionId,
  getTabs,
  setSessionTabRunId,
  setSessionTabSessionId,
  setSessionTabStatus
} from '$lib/features/workbench/state.svelte'

/**
 * Open a dormant session tab for `folderPath`. The mounted
 * SessionSurface spawns the process; nothing is persisted beyond the
 * tab itself.
 */
export function startSession(folderPath: string, harnessId: string | null, preset: string | null): TabId {
  const title =
    harnessId === null
      ? 'Terminal'
      : launchLabel(getHarness(folderPath, harnessId)?.label ?? harnessId, preset) + (preset === null ? ' session' : '')
  return addSessionTab(folderPath, { harnessId, preset, title, sessionId: null })
}

/** Live agent sessions (terminals excluded) in `folderPath`; above one, they share its working tree. */
export function countLiveAgents(folderPath: string): number {
  return getTabs().filter(
    (t) => t.kind === 'session' && t.folderPath === folderPath && t.harnessId !== null && isProcessLive(t.status)
  ).length
}

/**
 * Spawn the process behind a session tab. Snapshot the session id
 * *before* spawning: the backend may bind a session id during start, so
 * reading `tab.sessionId` afterwards can't tell "resumed" from "just
 * bound". `clearSessionTabSessionId` no-ops when discovery has
 * meanwhile set a different session id.
 */
export async function startSessionProcess(manager: TerminalSessionManager, tab: SessionTab): Promise<void> {
  const supplied = tab.sessionId
  setSessionTabStatus(tab.id, 'starting')
  const info = await manager.startPty({
    folderPath: tab.folderPath,
    harnessId: tab.harnessId,
    preset: tab.preset,
    sessionId: supplied
  })
  if (info.sessionId) {
    setSessionTabSessionId(tab.id, info.sessionId)
  } else if (supplied !== null) {
    clearSessionTabSessionId(tab.id, supplied)
    notify.info('Started a new conversation', 'The previous one no longer exists.')
  }
}

export async function restartSessionProcess(tab: SessionTab): Promise<void> {
  const manager = sessionRegistry.getOrCreate(tab.id)
  if (manager.isPtyActive()) await manager.stopPty()
  await startSessionProcess(manager, tab)
}

/** Stop a mounted session or its saved, currently unmounted run. */
export async function stopSessionProcess(tabId: TabId): Promise<void> {
  const manager = sessionRegistry.get(tabId)
  if (manager) return manager.stopPty()
  const tab = getTabs().find((candidate) => candidate.id === tabId)
  if (tab?.kind !== 'session' || !tab.runId) return
  await backend.sessions.stop(tab.runId)
  setSessionTabRunId(tabId, null)
  setSessionTabStatus(tabId, 'exited')
}
