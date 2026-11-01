<script lang="ts">
  import type { Snippet } from 'svelte'
  import { IconButton } from '$lib/components/ui/button'
  import { ArrowDown } from '$lib/icons/lucideExports'
  import { cn } from '$lib/utils/cn'

  let { children, footer }: { children?: Snippet; footer?: Snippet } = $props()

  let viewport: HTMLDivElement
  let atBottom = $state(true)

  function updateAtBottom() {
    atBottom = viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight <= 32
  }

  export function scrollToBottom(): void {
    atBottom = true
    if (viewport) viewport.scrollTop = viewport.scrollHeight
  }

  export function scrollPage(direction: 1 | -1): void {
    if (!viewport) return
    viewport.scrollBy({ top: direction * viewport.clientHeight * 0.9 })
    updateAtBottom()
  }

  // Watches the whole column, so a growing transcript or composer both keep the view pinned.
  function pinToBottom(column: HTMLDivElement) {
    scrollToBottom()
    const observer = new ResizeObserver(() => {
      if (atBottom) viewport.scrollTop = viewport.scrollHeight
    })
    observer.observe(column)
    return () => observer.disconnect()
  }
</script>

<!-- One scroller; the footer sticks inside the same column, so both share its width and scrollbar. -->
<div
  data-slot="conversation"
  bind:this={viewport}
  class="min-h-0 flex-1 overflow-y-auto px-4"
  onscroll={updateAtBottom}
>
  <div class="mx-auto flex min-h-full w-full max-w-5xl flex-col" {@attach pinToBottom}>
    <div class="flex flex-1 flex-col justify-end gap-4 pt-4 pb-2">
      {#if children}{@render children()}{/if}
    </div>
    <!-- before: fades the transcript as it scrolls under; at the bottom there is nothing under it, so it hides and
         the last line can sit close. The ground shadow covers the subpixel gap under the sticky edge without
         adding scroll height the way an overflowing box would. -->
    <div
      class={cn(
        'before:duration-fast sticky bottom-0 bg-ground pb-4 shadow-[0_4px_0_var(--color-ground)] before:pointer-events-none before:absolute before:inset-x-0 before:bottom-full before:h-10 before:bg-linear-to-t before:from-ground before:from-25% before:to-transparent before:transition-opacity',
        atBottom && 'before:opacity-0'
      )}
    >
      {#if !atBottom}
        <IconButton
          size="md"
          tooltip="Scroll to Bottom"
          tooltipSide="top"
          onclick={scrollToBottom}
          class="absolute bottom-full left-1/2 mb-2 -translate-x-1/2 border border-edge bg-raised shadow-popover"
        >
          <ArrowDown size={14} />
        </IconButton>
      {/if}
      {#if footer}{@render footer()}{/if}
    </div>
  </div>
</div>
