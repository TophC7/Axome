<script lang="ts">
  import type { HTMLAttributes } from 'svelte/elements'
  import { cn } from '$lib/utils/cn'

  let { text, class: className, ...rest }: HTMLAttributes<HTMLDivElement> & { text: string } = $props()
</script>

<div data-slot="message-user" class={cn('group', className)} {...rest}>
  <div
    class="ml-auto max-w-[85%] rounded-lg border border-edge bg-raised px-3 py-2 text-base break-words whitespace-pre-wrap text-fg"
  >
    {text}
  </div>
</div>
