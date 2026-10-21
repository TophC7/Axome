// Task lifecycle orchestration.
//
// The store (state.svelte.ts) is the cache; this module is the
// behavior layer: opening task tabs, handling singleton semantics,
// showing the confirm prompt, and rebinding singleton tabs on restart.

import { confirmAsync } from '$lib/features/confirm/service.svelte'
import { notify } from '$lib/features/notifications/state.svelte'
import { getErrorMessage } from '$lib/utils/client-error'
import type { TaskDefinition } from '$lib/types/backend'
import type { TabId, TaskTab } from '$lib/features/workbench/model'
import {
  addTaskTab,
  findTaskTabByTaskId,
  getActiveTab,
  getTabs,
  resetTaskTabForRestart
} from '$lib/features/workbench/state.svelte'
import { findTask } from '$lib/features/tasks/state.svelte'
import * as terminals from '$lib/features/terminal/registry'
import { stopTerminalProcess } from '$lib/features/terminal/lifecycle'

/** Start a task without mounting its surface; resolves at PTY readiness, not exit. */
export async function startTaskTab(tabId: TabId, clearBeforeStart = false): Promise<void> {
  const tab = getTabs().find((tab) => tab.id === tabId)
  if (!tab || tab.kind !== 'task') throw new Error('Task tab is no longer available')
  await terminals.getOrCreate(tab).start(clearBeforeStart)
}

// Tracks the most recently launched task per folder so "Re-run Last
// Task" in the palette can fire without re-prompting the user to pick.
const lastTaskByFolder = new Map<string, string>()

function rememberLastTask(folderPath: string, taskId: string): void {
  lastTaskByFolder.set(folderPath, taskId)
}

export function getLastTaskId(folderPath: string): string | null {
  return lastTaskByFolder.get(folderPath) ?? null
}

/** Path of the active text tab, only when it belongs to `folderPath`
 *  so a task never receives a file from another folder. */
function activeFilePathFor(folderPath: string): string | null {
  const active = getActiveTab()
  if (active?.kind !== 'text' || active.folderPath !== folderPath) return null
  return active.filePath
}

const trimmedOrNull = (s?: string | null) => s?.trim() || null

async function confirmIfRequired(task: TaskDefinition): Promise<boolean> {
  if (!task.confirm) return true
  return confirmAsync({
    title: 'Run task',
    message: `Run "${task.label}"?`,
    confirmLabel: 'Run',
    cancelLabel: 'Cancel'
  })
}

async function rebindTaskTab(tab: TaskTab, task: TaskDefinition, activeFilePath: string | null): Promise<TabId> {
  try {
    await stopTerminalProcess(tab)
  } catch (error) {
    notify.error('Stop task failed', getErrorMessage(error))
    throw error
  }
  terminals.dispose(tab.id)
  resetTaskTabForRestart(tab.id, crypto.randomUUID(), {
    activeFilePath,
    label: task.label,
    icon: trimmedOrNull(task.icon),
    group: trimmedOrNull(task.group)
  })
  rememberLastTask(tab.folderPath, task.id)
  return tab.id
}

export async function restartTaskTab(tab: TaskTab, task: TaskDefinition): Promise<TabId | null> {
  if (!(await confirmIfRequired(task))) return null
  return rebindTaskTab(tab, task, tab.activeFilePath)
}

/**
 * Open (or activate) a task tab for the given definition.
 *
 * Behavior:
 * - `confirm: true` → prompt before running. Cancel leaves state unchanged.
 * - `singleton: true` + existing live tab → rebind that tab to a
 *   fresh runId, reset its status, and activate it. Honors `clearOnRerun`.
 * - Otherwise → spawn a new task tab.
 */
export async function openTaskTab(
  folderPath: string,
  task: TaskDefinition,
  options: { activeFilePath?: string | null } = {}
): Promise<TabId | null> {
  if (!(await confirmIfRequired(task))) return null

  const icon = trimmedOrNull(task.icon)
  const group = trimmedOrNull(task.group)

  if (task.singleton) {
    const existing = findTaskTabByTaskId(folderPath, task.id)
    if (existing) {
      const activeFilePath = activeFilePathFor(folderPath) ?? options.activeFilePath ?? existing.activeFilePath
      return rebindTaskTab(existing, task, activeFilePath)
    }
  }

  const activeFilePath = activeFilePathFor(folderPath) ?? options.activeFilePath ?? null
  const runId = crypto.randomUUID()
  const tabId = addTaskTab(folderPath, {
    runId,
    taskId: task.id,
    activeFilePath,
    label: task.label,
    icon,
    group
  })
  rememberLastTask(folderPath, task.id)
  return tabId
}

/**
 * Launch the most recently run task in this folder. Returns null
 * when no prior task has been launched or the stored task id is no
 * longer present in `.sworm/tasks.jsonc`.
 */
export async function rerunLastTask(folderPath: string): Promise<TabId | null> {
  const taskId = getLastTaskId(folderPath)
  if (!taskId) return null
  const task = findTask(folderPath, taskId)
  return task ? openTaskTab(folderPath, task) : null
}
