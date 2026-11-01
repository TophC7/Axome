import { backend } from '$lib/api/backend'
import type { ToolStep } from '$lib/features/chat/turns'
import { notify } from '$lib/features/notifications/state.svelte'
import { resolveFileLocationPath } from '$lib/features/workbench/links/openLink'
import { getErrorMessage } from '$lib/utils/client-error'
import { basename } from '$lib/utils/paths'
import { record } from './payload'

/** Search output paths are relative to the tool root, except single-file grep's basename. */
export async function openReferenceResult(
  call: ToolStep,
  folderPath: string,
  resultPath: string,
  line: number | undefined,
  onOpenLocation: (path: string, line?: number) => void
): Promise<void> {
  try {
    const args = record(call.arguments)
    const root = await resolveFileLocationPath(typeof args.path === 'string' ? args.path : '.', folderPath)
    // Single-file grep reports the file's own basename; any other root is a directory holding the result.
    const fileRoot =
      call.name === 'grep' &&
      !resultPath.includes('/') &&
      basename(root) === resultPath &&
      !(await backend.folders.resolve(root).catch(() => null))
    onOpenLocation(fileRoot ? root : `${root.replace(/\/+$/, '')}/${resultPath}`, line)
  } catch (error) {
    notify.error('Cannot open reference', getErrorMessage(error))
  }
}
