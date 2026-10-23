<script lang="ts">
  import type { ComponentProps } from 'svelte'
  import * as Popover from '$lib/components/ui/popover'
  import { buttonVariants } from '$lib/components/ui/button'
  import { ProgressRing } from '$lib/components/ui/progress'
  import type { ChatStats } from '$lib/types/backend'
  import { formatCost, formatTokens } from '$lib/features/chat/format'
  import { cn } from '$lib/utils/cn'

  let {
    stats,
    class: className,
    ...rest
  }: Omit<ComponentProps<typeof Popover.Trigger>, 'child' | 'children'> & { stats: ChatStats } = $props()
  const fraction = $derived(stats.contextWindow > 0 ? stats.contextTokens / stats.contextWindow : 0)
</script>

<Popover.Root>
  <Popover.Trigger
    data-slot="context-usage"
    aria-label="Context Usage"
    class={cn(buttonVariants({ variant: 'ghost', size: 'xs' }), className)}
    {...rest}
  >
    <ProgressRing value={fraction} size={14} />
    <span class="font-mono text-xs text-muted">{Math.round(fraction * 100)}%</span>
  </Popover.Trigger>
  <Popover.Content side="top" class="w-56 p-3">
    <h3 class="mb-2 text-xs font-medium tracking-wide text-muted uppercase">Context</h3>
    <dl class="grid grid-cols-[1fr_auto] gap-x-3 gap-y-1 text-sm">
      <dt class="text-muted">Used</dt>
      <dd class="font-mono text-fg">{formatTokens(stats.contextTokens)}</dd>
      <dt class="text-muted">Window</dt>
      <dd class="font-mono text-fg">{stats.contextWindow > 0 ? formatTokens(stats.contextWindow) : 'Unknown'}</dd>
      <dt class="text-muted">Input</dt>
      <dd class="font-mono text-fg">{formatTokens(stats.input)}</dd>
      <dt class="text-muted">Output</dt>
      <dd class="font-mono text-fg">{formatTokens(stats.output)}</dd>
      <dt class="text-muted">Cache Read</dt>
      <dd class="font-mono text-fg">{formatTokens(stats.cacheRead)}</dd>
      <dt class="text-muted">Cache Write</dt>
      <dd class="font-mono text-fg">{formatTokens(stats.cacheWrite)}</dd>
      <dt class="text-muted">Cost</dt>
      <dd class="font-mono text-fg">{formatCost(stats.cost)}</dd>
    </dl>
  </Popover.Content>
</Popover.Root>
