<script lang="ts">
  import type { ComponentProps } from 'svelte'
  import * as Popover from '$lib/components/ui/popover'
  import { buttonVariants } from '$lib/components/ui/button'
  import { ProgressRing } from '$lib/components/ui/progress'
  import type { ChatUsage } from '$lib/types/backend'
  import { formatCost, formatTokens } from '$lib/features/chat/format'
  import { cn } from '$lib/utils/cn'

  let {
    usage,
    class: className,
    ...rest
  }: Omit<ComponentProps<typeof Popover.Trigger>, 'child' | 'children'> & { usage: ChatUsage | null } = $props()
</script>

{#if usage && usage.size !== 0}
  <Popover.Root>
    <Popover.Trigger
      data-slot="context-usage"
      aria-label="Context Usage"
      class={cn(buttonVariants({ variant: 'ghost', size: 'xs' }), className)}
      {...rest}
    >
      <ProgressRing value={usage.used / usage.size} size={14} />
      <span class="font-mono text-xs text-muted">{Math.round((usage.used / usage.size) * 100)}%</span>
    </Popover.Trigger>
    <Popover.Content side="top" class="w-56 p-3">
      <h3 class="mb-2 text-xs font-medium tracking-wide text-muted uppercase">Context</h3>
      <dl class="grid grid-cols-[1fr_auto] gap-x-3 gap-y-1 text-sm">
        <dt class="text-muted">Used</dt>
        <dd class="font-mono text-fg">{formatTokens(usage.used)}</dd>
        <dt class="text-muted">Window</dt>
        <dd class="font-mono text-fg">{formatTokens(usage.size)}</dd>
        <dt class="text-muted">Remaining</dt>
        <dd class="font-mono text-fg">{formatTokens(usage.size - usage.used)}</dd>
        {#if usage.cost}
          <dt class="text-muted">Cost</dt>
          <dd class="font-mono text-fg">{formatCost(usage.cost)}</dd>
        {/if}
      </dl>
    </Popover.Content>
  </Popover.Root>
{/if}
