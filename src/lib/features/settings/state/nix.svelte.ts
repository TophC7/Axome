// Per-folder Nix environment state using Svelte 5 runes.

import { SvelteSet } from 'svelte/reactivity'
import { backend } from '$lib/api/backend'
import { refreshLspFolderEnvironment } from '$lib/features/editor/lsp/registry'
import { createFolderKeyedStore } from '$lib/state/folderKeyedStore.svelte'
import { loadHarnessesForFolder } from '$lib/features/sessions/harnesses/state.svelte'
import { loadShellsForFolder } from '$lib/features/shells/state.svelte'
import { notify, dismissNotification } from '$lib/features/notifications/state.svelte'
import { getErrorMessage } from '$lib/utils/client-error'
import type { NixDetection } from '$lib/types/backend'
import type { ToneStatus } from '$lib/components/ui/status-dot'

const detections = createFolderKeyedStore<NixDetection>()
const evaluating = new SvelteSet<string>()
const evaluationNotifications = new Map<string, string>()

export function getNixDetection(folderPath: string): NixDetection | undefined {
  return detections.get(folderPath)
}

export function isNixEvaluating(folderPath: string): boolean {
  return evaluating.has(folderPath)
}

/** What the folder's Nix environment reads as at a glance; null when it has no Nix files. */
export function getNixStatus(folderPath: string): ToneStatus | null {
  const detection = getNixDetection(folderPath)
  if (!detection?.detected_files.length) return null
  const selected = detection.selected
  if (isNixEvaluating(folderPath) || selected?.status === 'evaluating') {
    return { tone: 'working', text: `Evaluating ${selected?.nix_file ?? 'Nix'}…` }
  }
  if (!selected) return { tone: 'idle', text: 'Nix available' }
  switch (selected.status) {
    case 'ready':
      return { tone: 'success', text: `Active · ${selected.nix_file}` }
    case 'error':
      return { tone: 'danger', text: 'Evaluation failed' }
    case 'timeout':
      return { tone: 'danger', text: 'Evaluation timed out' }
    default:
      return { tone: 'idle', text: `${selected.nix_file} not evaluated` }
  }
}

export async function detectNix(folderPath: string): Promise<NixDetection> {
  const generation = detections.generation(folderPath)
  const detection = await backend.nix.detect(folderPath)
  if (detections.generation(folderPath) === generation) detections.set(folderPath, detection)
  return detection
}

export function refreshNixForFolder(folderPath: string): void {
  if (detections.get(folderPath) != null) {
    const generation = detections.generation(folderPath)
    void detectNix(folderPath).catch((error) => {
      if (detections.generation(folderPath) === generation) {
        notify.error('Nix detection failed', getErrorMessage(error))
      }
    })
  }
}

export async function selectNixFile(folderPath: string, nixFile: string): Promise<void> {
  if (isNixEvaluating(folderPath)) return
  const generation = detections.generation(folderPath)
  const current = () => detections.generation(folderPath) === generation
  try {
    if (getNixDetection(folderPath)?.selected?.nix_file !== nixFile) {
      const record = await backend.nix.select(folderPath, nixFile)
      if (!current()) return
      detections.patch(folderPath, { selected: record })
      await refreshLspFolderEnvironment(folderPath)
      if (!current()) return
    }
    await evaluateNix(folderPath)
    if (!current()) return
  } catch (error) {
    if (current()) notify.error('Select Nix file failed', getErrorMessage(error))
  }
}

export async function evaluateNix(folderPath: string): Promise<void> {
  if (isNixEvaluating(folderPath)) return
  const generation = detections.generation(folderPath)
  const current = () => detections.generation(folderPath) === generation
  evaluating.add(folderPath)
  const notificationId = notify.loading('Evaluating Nix environment')
  evaluationNotifications.set(folderPath, notificationId)
  try {
    const record = await backend.nix.evaluate(folderPath)
    if (!current()) return
    detections.patch(folderPath, { selected: record })
    await refreshLspFolderEnvironment(folderPath)
    if (!current()) return
    await Promise.all([loadHarnessesForFolder(folderPath, current), loadShellsForFolder(folderPath, current)])
    if (!current()) return
    notify.update(notificationId, {
      title:
        record.status === 'ready'
          ? 'Nix environment ready'
          : record.status === 'timeout'
            ? 'Nix evaluation timed out'
            : 'Nix evaluation failed',
      description: record.status === 'ready' ? record.nix_file : (record.error_message ?? record.nix_file),
      tone: record.status === 'ready' ? 'success' : 'error',
      loading: false
    })
  } catch (error) {
    if (current()) {
      notify.update(notificationId, {
        title: 'Nix evaluation failed',
        description: getErrorMessage(error),
        tone: 'error',
        loading: false
      })
      await Promise.all([loadHarnessesForFolder(folderPath, current), loadShellsForFolder(folderPath, current)])
    }
  } finally {
    if (current()) {
      evaluating.delete(folderPath)
      evaluationNotifications.delete(folderPath)
    } else {
      dismissNotification(notificationId)
    }
  }
}

export async function clearNix(folderPath: string): Promise<void> {
  const generation = detections.generation(folderPath)
  const current = () => detections.generation(folderPath) === generation
  try {
    await backend.nix.clear(folderPath)
    if (!current()) return
    detections.patch(folderPath, { selected: null })
    await refreshLspFolderEnvironment(folderPath)
    if (!current()) return
    await Promise.all([loadHarnessesForFolder(folderPath, current), loadShellsForFolder(folderPath, current)])
    if (!current()) return
    notify.success('Disabled Nix environment')
  } catch (error) {
    if (current()) notify.error('Disable Nix environment failed', getErrorMessage(error))
  }
}

export async function activateNixFolder(folderPath: string): Promise<void> {
  const generation = detections.generation(folderPath)
  const current = () => detections.generation(folderPath) === generation
  try {
    const detection = await detectNix(folderPath)
    if (!current()) return
    if (detection.selected?.status === 'pending') {
      await evaluateNix(folderPath)
      if (!current()) return
    } else {
      // Folder presets and defaults apply even without a Nix environment.
      await Promise.all([loadHarnessesForFolder(folderPath, current), loadShellsForFolder(folderPath, current)])
      if (!current()) return
    }
  } catch (error) {
    if (current()) {
      notify.error('Nix detection failed', getErrorMessage(error))
      await Promise.all([loadHarnessesForFolder(folderPath, current), loadShellsForFolder(folderPath, current)])
    }
  }
}

/** Forget the folder's detection; called when the workbench releases the folder. */
export function releaseNixFolder(folderPath: string) {
  detections.delete(folderPath)
  evaluating.delete(folderPath)
  const notificationId = evaluationNotifications.get(folderPath)
  if (notificationId) dismissNotification(notificationId)
  evaluationNotifications.delete(folderPath)
}
