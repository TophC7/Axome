<script lang="ts">
  import type { ComponentProps } from 'svelte'
  import * as Collapsible from '$lib/components/ui/collapsible'
  import { Badge } from '$lib/components/ui/badge'
  import { ChevronRight, Circle, CircleCheck, ListTodoIcon, LoaderCircle } from '$lib/icons/lucideExports'
  import type { ChatPlanItem } from '$lib/types/backend'
  import { cn } from '$lib/utils/cn'

  let {
    items,
    open,
    onOpenChange,
    class: className,
    ...rest
  }: Omit<ComponentProps<typeof Collapsible.Root>, 'child' | 'children'> & {
    items: ChatPlanItem[]
    open: boolean
    onOpenChange: (open: boolean) => void
  } = $props()

  const completed = $derived(items.filter((item) => item.status === 'completed').length)
</script>

<Collapsible.Root
  data-slot="plan"
  {open}
  {onOpenChange}
  class={cn('rounded-lg border border-edge bg-surface', className)}
  {...rest}
>
  <Collapsible.Trigger
    class="flex h-8 w-full items-center gap-2 px-3 text-sm focus-visible:shadow-focus-ring focus-visible:outline-none"
  >
    <ListTodoIcon size={14} class="text-muted" />
    <span class="font-medium text-fg">Plan</span>
    <span class="font-mono text-xs text-subtle">{completed}/{items.length}</span>
    <ChevronRight size={12} class={cn('duration-fast ms-auto text-muted transition-transform', open && 'rotate-90')} />
  </Collapsible.Trigger>
  <Collapsible.Content>
    <div class="flex flex-col gap-1 px-3 pb-2">
      {#each items as item}
        <div class="flex items-start gap-2 text-sm">
          {#if item.status === 'completed'}
            <CircleCheck size={12} class="mt-0.5 shrink-0 text-success" />
          {:else if item.status === 'in_progress'}
            <LoaderCircle size={12} class="mt-0.5 shrink-0 animate-spin text-accent" />
          {:else}
            <Circle size={12} class="mt-0.5 shrink-0 text-subtle" />
          {/if}
          <span
            class={cn(
              item.status === 'completed'
                ? 'text-subtle line-through'
                : item.status === 'in_progress'
                  ? 'text-bright'
                  : 'text-fg'
            )}>{item.content}</span
          >
          {#if item.priority === 'high'}
            <Badge variant="warning">High</Badge>
          {/if}
        </div>
      {/each}
    </div>
  </Collapsible.Content>
</Collapsible.Root>
