<script lang="ts">
  import type { Snippet } from 'svelte'
  import { IconButton } from '$lib/components/ui/button'
  import { CopyIcon } from '$lib/icons/lucideExports'
  import type { ChatContent } from '$lib/types/backend'
  import { copyToClipboard } from '$lib/utils/clipboard'

  let { output, children }: { output: ChatContent[]; children: Snippet } = $props()

  // Copy joins the recorded output verbatim on demand, not the preview or footer-split view.
  const hasText = $derived(output.some((part) => part.type === 'text' && part.text.length > 0))
</script>

<div data-slot="terminal-output" class="flex min-w-0 items-start gap-2 border-t border-edge pt-2">
  <div class="flex min-w-0 flex-1 flex-col gap-1.5">
    {@render children()}
  </div>
  <IconButton
    tooltip="Copy Output"
    class="shrink-0"
    disabled={!hasText}
    onclick={() => void copyToClipboard(output.flatMap((part) => (part.type === 'text' ? [part.text] : [])).join('\n'))}
    ><CopyIcon size={12} /></IconButton
  >
</div>
