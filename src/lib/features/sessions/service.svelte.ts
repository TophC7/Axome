import { backend } from '$lib/api/backend'
import { notify } from '$lib/features/notifications/state.svelte'
import { getHarness, launchLabel } from '$lib/features/sessions/harnesses/catalog'
import * as chats from '$lib/features/chat/registry'
import * as terminals from '$lib/features/terminal/registry'
import { isTabInert } from '$lib/features/workbench/groups.svelte'
import { isChatTab, isProcessLive, type SessionTab, type Tab, type TabId } from '$lib/features/workbench/model'
import { stopTabRun } from '$lib/features/workbench/runLifecycle'
import { addSessionTab, getTabs, setSessionTabView } from '$lib/features/workbench/state.svelte'
import { isTabTransferring } from '$lib/features/workbench/transferService.svelte'
import type { SessionView } from '$lib/types/backend'
import { getErrorMessage } from '$lib/utils/client-error'

/** Open a dormant tab; the mounted surface starts it on activation. */
export function startSession(folderPath: string, harnessId: string, preset: string | null, view: SessionView): TabId {
  const title =
    launchLabel(getHarness(folderPath, harnessId)?.label ?? harnessId, preset) + (preset === null ? ' session' : '')
  return addSessionTab(folderPath, { harnessId, preset, view, title, sessionId: null })
}

export function canSwitchView(tab: Tab | null | undefined): tab is SessionTab {
  return (
    !!tab &&
    tab.kind === 'session' &&
    !tab.locked &&
    !isTabTransferring(tab.id) &&
    !!getHarness(tab.folderPath, tab.harnessId)?.chat &&
    !isTabInert(tab)
  )
}

/** Stop the run, then reopen the same session in the other view. Before the first chat turn there is no session id, so the other view starts fresh. */
export async function switchSessionView(tabId: TabId): Promise<void> {
  const tab = getTabs().find((tab) => tab.id === tabId)
  if (!canSwitchView(tab)) return
  await stopTabRun(tab)
  if (isChatTab(tab)) chats.release(tab.id)
  else terminals.dispose(tab.id)
  setSessionTabView(tab.id, tab.view === 'chat' ? 'terminal' : 'chat')
}

export function canForkSession(tab: Tab | null | undefined): tab is SessionTab {
  if (!canSwitchView(tab) || !isChatTab(tab) || !tab.runId) return false
  const state = chats.get(tab.id)?.state
  return state?.capabilities.fork === true && state.session_id !== null && state.turn === 'idle'
}

export async function forkSession(tabId: TabId): Promise<void> {
  const tab = getTabs().find((tab) => tab.id === tabId)
  if (!canForkSession(tab) || !tab.runId) return
  try {
    const { sessionId } = await backend.chat.fork(tab.runId)
    addSessionTab(tab.folderPath, {
      harnessId: tab.harnessId,
      preset: tab.preset,
      view: 'chat',
      title: tab.title,
      sessionId
    })
  } catch (error) {
    notify.error('Failed to fork session', getErrorMessage(error))
  }
}

/** Live agents above one share the folder's working tree. */
export function countLiveAgents(folderPath: string): number {
  return getTabs().filter((tab) => tab.kind === 'session' && tab.folderPath === folderPath && isProcessLive(tab.status))
    .length
}
