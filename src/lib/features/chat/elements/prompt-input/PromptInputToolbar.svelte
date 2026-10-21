<script lang="ts">
  import type { Snippet } from 'svelte'
  import type { HTMLAttributes } from 'svelte/elements'
  import { cn } from '$lib/utils/cn'

  let {
    start,
    end,
    class: className,
    ...rest
  }: HTMLAttributes<HTMLDivElement> & {
    start?: Snippet
    end?: Snippet
  } = $props()
</script>

<div data-slot="prompt-input-toolbar" class={cn('flex h-9 items-center gap-1 px-1.5', className)} {...rest}>
  {#if start}{@render start()}{/if}
  <div class="ms-auto flex items-center gap-1">
    {#if end}{@render end()}{/if}
  </div>
</div>
