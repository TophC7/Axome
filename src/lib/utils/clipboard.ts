import { notify } from '$lib/features/notifications/state.svelte'
import { platform } from '$lib/platform'
import { getErrorMessage } from '$lib/utils/client-error'

/** Copy text; clipboard refusal (permissions, unfocused page) notifies instead of rejecting. */
export async function copyToClipboard(text: string, errorTitle = 'Copy failed'): Promise<void> {
  try {
    await platform.clipboard.writeText(text)
  } catch (e) {
    notify.error(errorTitle, getErrorMessage(e))
  }
}
