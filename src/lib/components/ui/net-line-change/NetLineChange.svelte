<script lang="ts" module>
  import { tv, type VariantProps } from 'tailwind-variants'

  export const netLineChangeVariants = tv({
    base: 'shrink-0 font-mono tabular-nums',
    variants: { direction: { up: 'text-success', down: 'text-danger' } }
  })

  export type NetLineChangeDirection = VariantProps<typeof netLineChangeVariants>['direction']
</script>

<!--
  @component
  NetLineChange — added minus removed lines as one signed count (`+3`, `-2`).
  A change that nets to zero renders `fallback` instead (or nothing).
-->
<script lang="ts">
  import type { Snippet } from 'svelte'
  import type { HTMLAttributes } from 'svelte/elements'
  import { cn } from '$lib/utils/cn'

  let {
    added,
    removed,
    fallback,
    class: className,
    ...rest
  }: Omit<HTMLAttributes<HTMLSpanElement>, 'children'> & {
    added: number
    removed: number
    fallback?: Snippet
    class?: string
  } = $props()

  const net = $derived(added - removed)
</script>

{#if net !== 0}
  <span
    data-slot="net-line-change"
    class={cn(netLineChangeVariants({ direction: net > 0 ? 'up' : 'down' }), className)}
    {...rest}>{net > 0 ? '+' : ''}{net}</span
  >
{:else if fallback}
  {@render fallback()}
{/if}
