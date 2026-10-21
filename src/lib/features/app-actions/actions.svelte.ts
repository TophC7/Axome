// Shared app actions.
//
// Button clicks, command-palette entries, and global shortcuts all call
// these functions so confirmation and side effects stay on one path.

import { backend } from '$lib/api/backend'
import { platform, requireNative } from '$lib/platform'
import { confirmAsync } from '$lib/features/confirm/service.svelte'
import { notify } from '$lib/features/notifications/state.svelte'
import { getErrorMessage } from '$lib/utils/client-error'
import { runNotifiedTask } from '$lib/features/notifications/runNotifiedTask'
import {
  getLaunchOptions,
  reloadHarnessListings,
  resolveDefaultLaunch
} from '$lib/features/sessions/harnesses/state.svelte'
import { getHarness, launchLabel } from '$lib/features/sessions/harnesses/catalog'
import { startSession } from '$lib/features/sessions/service.svelte'
import { setSettingsOpen } from '$lib/features/settings/dialog/state.svelte'
import { getLastTaskId, rerunLastTask } from '$lib/features/tasks/service.svelte'
import { openCommandPaletteWithSearch } from '$lib/features/command-palette/state.svelte'
import {
  createUntitledTextSurface,
  confirmDiscardDirtyText,
  isTextSurfaceDirty,
  openTextFile
} from '$lib/features/workbench/surfaces/text/service.svelte'
import {
  flushWorkbench,
  getActiveFolderPath,
  getTabs,
  openFolder,
  reopenLastClosedTab
} from '$lib/features/workbench/state.svelte'
import { closeFocusedTab } from '$lib/features/workbench/tabActions.svelte'
import type { TabId } from '$lib/features/workbench/model'
import { takeBackGroup, moveGroupToNewWindow, removeGroupFromWindow } from '$lib/features/workbench/groups.svelte'
import { basename, resolveProjectFile, splitRemotePath } from '$lib/utils/paths'

/** Managed reload: confirm unsaved, flush persistence, then reload. */
export async function reloadView(): Promise<void> {
  // Web: the document's beforeunload guard owns the dirty prompt.
  if (platform.native && !(await confirmDiscardDirtyText('Reload'))) return
  try {
    await flushWorkbench()
  } catch (error) {
    console.warn('Reload flush failed:', error)
  }
  window.location.reload()
}

/** Rebuild the extension catalog on the active folder's host, then reload this view to apply it. */
export async function reloadExtensions(): Promise<void> {
  try {
    await backend.extensions.reload(getActiveFolderPath())
  } catch (error) {
    notify.error('Reload extensions failed', getErrorMessage(error))
    return
  }
  await reloadView()
}

/** Re-probe the active folder's host environment, then refresh harness detection. */
export async function reloadEnvironment(): Promise<void> {
  try {
    await backend.environment.reload(getActiveFolderPath())
    await reloadHarnessListings()
  } catch (error) {
    notify.error('Reload environment failed', getErrorMessage(error))
  }
}

export function confirmCloseWorkbench(folders: string[]): Promise<boolean> {
  return confirmAsync({
    title: 'Close Workbench',
    message: `Stop all sessions and tasks in ${[...new Set(folders.map(basename))].join(', ') || 'this workbench'} and delete its saved tabs and layout? Unsaved changes in any connected page will be lost.`,
    confirmLabel: 'Close Workbench',
    cancelLabel: 'Cancel'
  })
}

let closingWorkbench = false

/** Confirm, then ask the host to close (stop runs, delete state of) this workbench. */
export async function closeCurrentWorkbench(): Promise<void> {
  if (closingWorkbench) return
  closingWorkbench = true
  try {
    const closeCurrent = platform.workbench.closeCurrent
    if (!closeCurrent) throw new Error('Closing the current workbench is not supported here')
    const dirtyPaths = getTabs().flatMap((tab) =>
      tab.kind === 'text' && isTextSurfaceDirty(tab.id)
        ? [tab.filePath === null ? tab.fileName : resolveProjectFile(tab.folderPath, tab.filePath)]
        : []
    )
    const confirmed = await confirmAsync({
      title: 'Close Workbench',
      message: [
        'Stop all sessions and tasks in this workbench and delete its saved tabs and layout?',
        ...(dirtyPaths.length > 0 ? ['', 'Unsaved changes will be lost:', ...dirtyPaths] : [])
      ].join('\n'),
      confirmLabel: 'Close Workbench',
      cancelLabel: 'Cancel'
    })
    if (!confirmed) return
    await closeCurrent()
  } catch (error) {
    notify.error('Could not close workbench', getErrorMessage(error))
  } finally {
    closingWorkbench = false
  }
}

export async function newWindow(): Promise<void> {
  await requireNative().window.create()
}

export function takeOverServerWorkbench(server: string) {
  return runNotifiedTask(() => takeBackGroup(server), {
    loading: { title: 'Taking back workbench', description: server },
    error: { title: 'Take back failed' }
  })
}

export function moveServerWorkbenchToNewWindow(server: string) {
  return runNotifiedTask(() => moveGroupToNewWindow(server), {
    loading: { title: 'Moving workbench', description: server },
    error: { title: 'Move workbench failed' }
  })
}

export function removeServerWorkbenchFromWindow(server: string) {
  return runNotifiedTask(() => removeGroupFromWindow(server), {
    loading: { title: 'Removing workbench', description: server },
    error: { title: 'Remove workbench failed' }
  })
}

export function newEmptyFile(): void {
  const folderPath = getActiveFolderPath()
  if (!folderPath) return
  createUntitledTextSurface(folderPath)
}

/** Native directory picker → open (or focus) that folder. */
export async function openFolderPicker(replaceTabId?: TabId): Promise<void> {
  try {
    const path = await requireNative().dialogs.selectDirectory()
    if (path) await openFolder(path, replaceTabId)
  } catch (error) {
    notify.error('Open folder failed', getErrorMessage(error))
  }
}

export function openSettings(): void {
  setSettingsOpen(true)
}

export async function openGlobalSettingsFile(): Promise<void> {
  try {
    await requireNative().settings.openGlobalFile()
  } catch (error) {
    notify.error('Open Global Settings failed', getErrorMessage(error))
  }
}

export async function openFolderSettingsFile(): Promise<void> {
  const folderPath = getActiveFolderPath()
  if (!folderPath) {
    notify.error('No active folder', 'Open a folder before opening folder settings.')
    return
  }

  try {
    await backend.settings.openFolderFile(folderPath)
    await openTextFile(folderPath, '.sworm/settings.jsonc', { temporary: false })
  } catch (error) {
    notify.error('Open Folder Settings failed', getErrorMessage(error))
  }
}

export function revealActiveFolderInFileManager(): void {
  const folderPath = getActiveFolderPath()
  if (!folderPath || splitRemotePath(folderPath)) return
  void requireNative()
    .files.reveal(folderPath)
    .catch((error) => {
      notify.error('Reveal in file manager failed', getErrorMessage(error))
    })
}

export function openActiveFolderInExternalTerminal(): void {
  const folderPath = getActiveFolderPath()
  if (!folderPath) return
  void requireNative()
    .files.openInTerminal(folderPath)
    .catch((error) => {
      notify.error('Open in terminal failed', getErrorMessage(error))
    })
}

export function createSession(harnessId: string, preset: string | null): void {
  const folderPath = getActiveFolderPath()
  if (!folderPath) return
  const option = getLaunchOptions(folderPath).find((o) => o.harness.id === harnessId && o.preset === preset)
  const label = option?.label ?? launchLabel(getHarness(folderPath, harnessId)?.label ?? harnessId, preset)
  if (option?.status?.status !== 'connected') {
    notify.error(`${label} unavailable`, `Install or enable ${label} in Settings → Harnesses first.`)
    return
  }
  startSession(folderPath, harnessId, preset)
}

export function newTerminalSession(): void {
  const folderPath = getActiveFolderPath()
  if (!folderPath) return
  startSession(folderPath, null, null)
}

export function newDefaultSession(): void {
  const folderPath = getActiveFolderPath()
  if (!folderPath) return
  const { option, configured } = resolveDefaultLaunch(folderPath)
  if (configured !== null && option === null) {
    notify.error('Default preset unavailable', `${configured} is not a connected harness preset in this folder.`)
    return
  }
  const launch = option ?? getLaunchOptions(folderPath).find((o) => o.status?.status === 'connected')
  if (!launch) {
    notify.error('No harness available', 'Install a harness or enable one in Settings → Harnesses.')
    return
  }
  startSession(folderPath, launch.harness.id, launch.preset)
}

export function showTasks(): void {
  openCommandPaletteWithSearch('! ')
}

/**
 * Opens the command palette in file-search mode. Bound to Ctrl+P,
 * matching VS Code's Quick Open.
 */
export function showFiles(): void {
  openCommandPaletteWithSearch('/ ')
}

export async function rerunLastFolderTask(): Promise<void> {
  const folderPath = getActiveFolderPath()
  if (!folderPath) return
  if (!getLastTaskId(folderPath)) return
  const tabId = await rerunLastTask(folderPath)
  if (tabId === null) {
    notify.error('Cannot re-run task', 'The last task is no longer defined in .sworm/tasks.jsonc')
  }
}

export async function closeActiveTab(): Promise<void> {
  try {
    await closeFocusedTab()
  } catch (error) {
    notify.error('Close tab failed', getErrorMessage(error))
  }
}

export function reopenTab(): void {
  void reopenLastClosedTab().catch((e) => notify.error('Reopen tab failed', getErrorMessage(e)))
}
