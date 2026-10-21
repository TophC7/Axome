import { backend } from '$lib/api/backend'
import { notify } from '$lib/features/notifications/state.svelte'
import { getHarness } from '$lib/features/sessions/harnesses/catalog'
import { findTask } from '$lib/features/tasks/state.svelte'
import type { TerminalTab } from '$lib/features/workbench/model'
import {
  clearSessionTabSessionId,
  getTabs,
  persistRunTabRunId,
  setSessionTabSessionId,
  setTaskTabStatus,
  setRunTabRunId,
  setRunTabStatus,
  setTerminalTabTitle
} from '$lib/features/workbench/state.svelte'
import type { PtyEvent, ShiftEncoding } from '$lib/types/backend'
import type { TerminalManager } from './TerminalManager'
import { stripTitlePrefixes } from './terminalKeymap'

export interface TerminalBehavior {
  start(manager: TerminalManager): Promise<void>
  stop(runId: string): Promise<void>
  shiftEncoding(): ShiftEncoding | null
  onTitle: ((title: string) => void) | null
  onEvent(event: PtyEvent): void
  onPtyLost(): void
  renderErrors: boolean
  canInteract(): boolean
  startPolicy: 'debounced' | 'immediate'
}

export function behaviorFor(tab: TerminalTab): TerminalBehavior {
  if (tab.kind === 'task') {
    return {
      startPolicy: 'immediate',
      renderErrors: true,
      canInteract: () => {
        const current = getTabs().find((candidate) => candidate.id === tab.id)
        return current?.kind === 'task' && current.status === 'running'
      },
      onPtyLost: () => setTaskTabStatus(tab.id, 'failed', null),
      shiftEncoding: () => 'legacy',
      onTitle: null,
      stop: (runId) => backend.tasks.stop(runId),
      async start(manager) {
        setTaskTabStatus(tab.id, 'starting', null)
        await manager.openRun(
          tab.runId,
          (cols, rows, sinks) =>
            backend.tasks.start(
              tab.runId,
              tab.folderPath,
              tab.taskId,
              tab.activeFilePath,
              cols,
              rows,
              tab.attachOnly,
              sinks
            ),
          findTask(tab.folderPath, tab.taskId)?.clearOnRerun ?? false
        )
      },
      onEvent(event) {
        if (event.type === 'started') setTaskTabStatus(tab.id, 'running', null)
        else if (event.type === 'exit') setTaskTabStatus(tab.id, 'exited', event.code ?? null)
        else if (event.type === 'error') setTaskTabStatus(tab.id, 'failed', null)
      }
    }
  }

  return {
    startPolicy: 'debounced',
    renderErrors: false,
    canInteract: () => true,
    onPtyLost() {
      setRunTabRunId(tab.id, null)
      setRunTabStatus(tab.id, 'failed')
    },
    shiftEncoding: () =>
      tab.kind === 'session' ? (getHarness(tab.folderPath, tab.harnessId)?.shift_encoding ?? 'legacy') : 'legacy',
    onTitle: (title) =>
      setTerminalTabTitle(
        tab.id,
        tab.kind === 'session'
          ? stripTitlePrefixes(title, getHarness(tab.folderPath, tab.harnessId)?.title_prefixes ?? [])
          : title
      ),
    stop: (runId) => (tab.kind === 'session' ? backend.sessions.stop(runId) : backend.shells.stop(runId)),
    async start(manager) {
      const current = getTabs().find((candidate) => candidate.id === tab.id)
      if (!current || (current.kind !== 'session' && current.kind !== 'shell')) return
      const runId = current.runId ?? crypto.randomUUID()
      setRunTabStatus(tab.id, 'starting')
      await persistRunTabRunId(tab.id, runId)
      if (current.kind === 'shell') {
        await manager.openRun(runId, (cols, rows, sinks) =>
          backend.shells.start({ runId, folderPath: current.folderPath, preset: current.preset }, cols, rows, sinks)
        )
        return
      }
      // Capture before discovery can bind a different session id during startup.
      const supplied = current.sessionId
      const info = await manager.openRun(runId, (cols, rows, sinks) =>
        backend.sessions.start(
          {
            runId,
            folderPath: current.folderPath,
            harnessId: current.harnessId,
            preset: current.preset,
            sessionId: supplied
          },
          cols,
          rows,
          sinks
        )
      )
      if (info.sessionId) setSessionTabSessionId(tab.id, info.sessionId)
      else if (supplied !== null) {
        clearSessionTabSessionId(tab.id, supplied)
        notify.info('Started a new conversation', 'The previous one no longer exists.')
      }
    },
    onEvent(event) {
      if (event.type === 'started') setRunTabStatus(tab.id, 'running')
      else if (event.type === 'exit') {
        setRunTabRunId(tab.id, null)
        setRunTabStatus(tab.id, 'exited')
      } else if (event.type === 'error') {
        setRunTabStatus(tab.id, 'failed')
      } else if (tab.kind === 'session' && event.type === 'sessionIdBound' && event.session_id) {
        setSessionTabSessionId(tab.id, event.session_id)
      }
    }
  }
}
