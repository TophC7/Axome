<script lang="ts">
  import type { ComponentProps } from 'svelte'
  import * as Collapsible from '$lib/components/ui/collapsible'
  import { StatusDot } from '$lib/components/ui/status-dot'
  import { BrainIcon, ChevronRight } from '$lib/icons/lucideExports'
  import { cn } from '$lib/utils/cn'

  let {
    text,
    streaming,
    seconds,
    open,
    onOpenChange,
    class: className,
    ...rest
  }: Omit<ComponentProps<typeof Collapsible.Root>, 'child' | 'children'> & {
    text: string
    streaming: boolean
    seconds?: number
    open: boolean
    onOpenChange: (open: boolean) => void
  } = $props()
</script>

<Collapsible.Root data-slot="reasoning" {open} {onOpenChange} class={cn(className)} {...rest}>
  <Collapsible.Trigger
    class="flex h-7 items-center gap-1.5 text-sm text-muted transition-colors hover:text-bright focus-visible:shadow-focus-ring focus-visible:outline-none"
  >
    <ChevronRight size={12} class={cn('duration-fast transition-transform', open && 'rotate-90')} />
    <BrainIcon size={14} />
    {#if streaming}
      <span class="animate-pulse">Thinking</span>
      <StatusDot tone="working" size="sm" />
    {:else if seconds !== undefined}
      <span>Thought for {seconds}s</span>
    {:else}
      <span>Thoughts</span>
    {/if}
  </Collapsible.Trigger>
  <Collapsible.Content class="ml-1.5 border-l border-edge pl-3 text-sm whitespace-pre-wrap text-muted">
    {text}
  </Collapsible.Content>
</Collapsible.Root>
