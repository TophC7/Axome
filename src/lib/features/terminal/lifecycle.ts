import { notify } from '$lib/features/notifications/state.svelte'
import { restartTaskTab } from '$lib/features/tasks/service.svelte'
import { findTask } from '$lib/features/tasks/state.svelte'
import type { TerminalTab } from '$lib/features/workbench/model'
import { behaviorFor } from './behaviors'
import * as terminals from './registry'

export async function stopTerminalProcess(tab: TerminalTab): Promise<void> {
  const manager = terminals.get(tab.id)
  if (manager) return manager.stopPty()
  if (!tab.runId) return
  const behavior = behaviorFor(tab)
  await behavior.stop(tab.runId)
  behavior.onEvent({ type: 'exit', run_id: tab.runId })
}

export async function restartTerminalProcess(tab: TerminalTab): Promise<void> {
  if (tab.kind === 'task') {
    const task = findTask(tab.folderPath, tab.taskId)
    if (!task) {
      notify.error('Cannot restart task', `Task "${tab.taskId}" is no longer defined in .sworm/tasks.jsonc`)
      return
    }
    await restartTaskTab(tab, task)
    return
  }
  const manager = terminals.getOrCreate(tab)
  if (manager.isPtyActive()) await manager.stopPty()
  await manager.start()
}
