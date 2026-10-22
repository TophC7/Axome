<script lang="ts">
  import type { ComponentProps, Snippet } from 'svelte'
  import * as Collapsible from '$lib/components/ui/collapsible'
  import { ChevronRight } from '$lib/icons/lucideExports'
  import { cn } from '$lib/utils/cn'

  let {
    summary,
    open,
    onOpenChange,
    children,
    class: className,
    ...rest
  }: Omit<ComponentProps<typeof Collapsible.Root>, 'child' | 'children'> & {
    summary: string
    open: boolean
    onOpenChange: (open: boolean) => void
    children?: Snippet
  } = $props()
</script>

<Collapsible.Root data-slot="turn-fold" {open} {onOpenChange} class={cn('min-w-0', className)} {...rest}>
  <Collapsible.Trigger
    class="flex w-full items-center gap-1.5 py-1.5 text-left text-xs text-muted hover:text-bright focus-visible:shadow-focus-ring focus-visible:outline-none"
  >
    <ChevronRight size={12} class={cn('shrink-0', open && 'rotate-90')} />
    <span>{summary}</span>
  </Collapsible.Trigger>
  <Collapsible.Content class="flex min-w-0 flex-col gap-2 pt-2">
    {#if children}{@render children()}{/if}
  </Collapsible.Content>
</Collapsible.Root>
