<script lang="ts">
  import type { ComponentProps } from 'svelte'
  import * as Collapsible from '$lib/components/ui/collapsible'
  import { ChevronRight, LightbulbIcon } from '$lib/icons/lucideExports'
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

  const preview = $derived(text.match(/^\s*(\S[^\r\n]*)/m)?.[1]?.trim() ?? '')
</script>

<Collapsible.Root
  data-slot="reasoning"
  {open}
  {onOpenChange}
  class={cn('rounded-md border border-edge bg-surface px-2.5 py-1.5 font-mono text-xs text-muted', className)}
  {...rest}
>
  <Collapsible.Trigger
    aria-label={streaming ? 'Thinking' : 'Thoughts'}
    class="flex w-full min-w-0 items-center gap-1.5 text-left hover:text-bright focus-visible:shadow-focus-ring focus-visible:outline-none"
  >
    <LightbulbIcon size={12} class={cn('shrink-0', streaming && 'text-accent')} />
    <span class="min-w-0 flex-1 truncate">{!open && preview ? preview : streaming ? 'Thinking' : 'Thoughts'}</span>
    {#if seconds !== undefined}
      <span class="shrink-0 text-subtle tabular-nums">{seconds}s</span>
    {/if}
    <ChevronRight size={12} class={cn('duration-fast shrink-0 transition-transform', open && 'rotate-90')} />
  </Collapsible.Trigger>
  <Collapsible.Content class="pt-1.5 whitespace-pre-wrap">
    {text}
  </Collapsible.Content>
</Collapsible.Root>
