<script lang="ts">
  import type { HTMLAttributes } from 'svelte/elements'
  import { IconButton } from '$lib/components/ui/button'
  import { CopyIcon } from '$lib/icons/lucideExports'
  import { copyToClipboard } from '$lib/utils/clipboard'
  import { cn } from '$lib/utils/cn'

  let { text, class: className, ...rest }: HTMLAttributes<HTMLDivElement> & { text: string } = $props()
</script>

<div
  data-slot="message-actions"
  class={cn(
    'duration-fast flex h-6 items-center opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100',
    className
  )}
  {...rest}
>
  <IconButton tooltip="Copy Message" onclick={() => copyToClipboard(text, 'Copy message failed')}>
    <CopyIcon size={12} />
  </IconButton>
</div>
