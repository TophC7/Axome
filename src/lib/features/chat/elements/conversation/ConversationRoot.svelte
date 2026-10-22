<script lang="ts">
  import type { Snippet } from 'svelte'
  import type { HTMLAttributes } from 'svelte/elements'
  import { IconButton } from '$lib/components/ui/button'
  import { ArrowDown } from '$lib/icons/lucideExports'
  import { cn } from '$lib/utils/cn'

  let { children, class: className, ...rest }: HTMLAttributes<HTMLDivElement> & { children?: Snippet } = $props()

  let viewport: HTMLDivElement
  let atBottom = $state(true)

  export function scrollToBottom(): void {
    atBottom = true
    if (viewport) viewport.scrollTop = viewport.scrollHeight
  }

  function observeContent(content: HTMLDivElement) {
    scrollToBottom()
    const observer = new ResizeObserver(() => {
      if (atBottom && viewport) viewport.scrollTop = viewport.scrollHeight
    })
    observer.observe(content)
    return () => observer.disconnect()
  }
</script>

<div data-slot="conversation" class={cn('relative flex min-h-0 flex-1 flex-col', className)} {...rest}>
  <div
    bind:this={viewport}
    class="min-h-0 flex-1 overflow-y-auto px-4"
    onscroll={(event) => {
      const viewport = event.currentTarget
      atBottom = viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight <= 32
    }}
  >
    <div class="mx-auto flex w-full max-w-3xl flex-col gap-4 py-4" {@attach observeContent}>
      {#if children}{@render children()}{/if}
    </div>
  </div>
  {#if !atBottom}
    <IconButton
      size="md"
      tooltip="Scroll to Bottom"
      onclick={scrollToBottom}
      class="absolute bottom-3 left-1/2 -translate-x-1/2 border border-edge bg-raised shadow-popover"
    >
      <ArrowDown size={14} />
    </IconButton>
  {/if}
</div>
