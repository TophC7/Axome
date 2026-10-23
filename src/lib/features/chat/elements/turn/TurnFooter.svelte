<script lang="ts">
  import type { HTMLAttributes } from 'svelte/elements'
  import { formatCost, formatTokens } from '$lib/features/chat/format'
  import type { Turn } from '$lib/features/chat/turns'
  import { cn } from '$lib/utils/cn'

  let { turn, class: className, ...rest }: HTMLAttributes<HTMLDivElement> & { turn: Turn } = $props()
  const parts = $derived.by(() => {
    if (!turn.assistants.length) return ''
    const usage = { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, cost: 0 }
    for (const entry of turn.assistants) {
      usage.input += entry.body.usage.input
      usage.output += entry.body.usage.output
      usage.cacheRead += entry.body.usage.cacheRead
      usage.cacheWrite += entry.body.usage.cacheWrite
      usage.cost += entry.body.usage.cost
    }
    return [
      `${formatTokens(usage.input)} in`,
      `${formatTokens(usage.output)} out`,
      usage.cacheRead ? `${formatTokens(usage.cacheRead)} cache R` : '',
      usage.cacheWrite ? `${formatTokens(usage.cacheWrite)} cache W` : '',
      formatCost(usage.cost)
    ]
      .filter(Boolean)
      .join(' · ')
  })
</script>

{#if parts}
  <div
    data-slot="turn-footer"
    class={cn('flex min-h-6 items-center gap-2 font-mono text-xs text-subtle', className)}
    {...rest}
  >
    {parts}
  </div>
{/if}
