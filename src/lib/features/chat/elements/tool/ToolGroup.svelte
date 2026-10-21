<script lang="ts">
  import type { ComponentProps, Snippet } from 'svelte'
  import * as Collapsible from '$lib/components/ui/collapsible'
  import { StatusDot, type StatusTone } from '$lib/components/ui/status-dot'
  import { ChevronRight } from '$lib/icons/lucideExports'
  import { cn } from '$lib/utils/cn'

  let {
    count,
    summary,
    tone,
    open,
    onOpenChange,
    children,
    class: className,
    ...rest
  }: Omit<ComponentProps<typeof Collapsible.Root>, 'child' | 'children'> & {
    count: number
    summary: string
    tone: StatusTone
    open: boolean
    onOpenChange: (open: boolean) => void
    children?: Snippet
  } = $props()
</script>

<Collapsible.Root data-slot="tool-group" {open} {onOpenChange} class={cn('flex flex-col', className)} {...rest}>
  <Collapsible.Trigger
    class="flex h-7 w-full items-center gap-2 px-1.5 text-sm hover:bg-surface focus-visible:shadow-focus-ring focus-visible:outline-none"
  >
    <ChevronRight size={12} class={cn('duration-fast shrink-0 text-muted transition-transform', open && 'rotate-90')} />
    <span class="shrink-0 text-fg">{count} tool calls</span>
    <span class="truncate text-subtle">{summary}</span>
    <StatusDot
      {tone}
      label={tone === 'working'
        ? 'Running'
        : tone === 'waiting'
          ? 'Awaiting approval'
          : tone === 'success'
            ? 'Completed'
            : tone === 'danger'
              ? 'Failed'
              : 'Pending'}
      class={cn('ms-auto', tone === 'working' && 'animate-pulse')}
    />
  </Collapsible.Trigger>
  <Collapsible.Content class="ml-2.5 flex flex-col border-l border-edge pl-2">
    {#if children}{@render children()}{/if}
  </Collapsible.Content>
</Collapsible.Root>
