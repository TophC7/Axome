<script lang="ts">
  import type { ComponentProps, Snippet } from 'svelte'
  import type { ChatToolContent, ChatToolStatus, ChatLocation } from '$lib/types/backend'
  import * as Collapsible from '$lib/components/ui/collapsible'
  import { StatusDot } from '$lib/components/ui/status-dot'
  import { ChevronRight } from '$lib/icons/lucideExports'
  import { basename } from '$lib/utils/paths'
  import { cn } from '$lib/utils/cn'
  import { toolKindIcon, toolStatusTone } from './kinds'
  import ToolDiff from './ToolDiff.svelte'

  let {
    title,
    tool_kind,
    status,
    content,
    locations,
    awaiting,
    open,
    onOpenChange,
    onOpenLocation,
    folderPath,
    children,
    class: className,
    ...rest
  }: Omit<ComponentProps<typeof Collapsible.Root>, 'child' | 'children' | 'content'> & {
    title: string
    tool_kind: string
    status: ChatToolStatus
    content: ChatToolContent[]
    locations: ChatLocation[]
    awaiting: boolean
    open: boolean
    onOpenChange: (open: boolean) => void
    onOpenLocation: (path: string, line?: number) => void
    folderPath: string
    children?: Snippet
  } = $props()
  const Icon = $derived(toolKindIcon(tool_kind))
  const state = $derived(toolStatusTone(status, awaiting))
  const expandable = $derived(content.length > 0 || locations.length > 0)
  const firstLocation = $derived(locations[0])
</script>

{#snippet row()}
  <span class="w-3 shrink-0">
    {#if expandable}<ChevronRight
        size={12}
        class={cn('duration-fast text-muted transition-transform', open && 'rotate-90')}
      />{/if}
  </span>
  <Icon size={14} class="shrink-0 text-muted" />
  <span class="truncate text-fg">{title}</span>
  {#if firstLocation}
    <span class="truncate font-mono text-xs text-subtle"
      >{basename(firstLocation.path)}{firstLocation.line != null ? `:${firstLocation.line}` : ''}</span
    >
  {/if}
  <StatusDot tone={state.tone} label={state.label} class={cn('ms-auto', state.pulse && 'animate-pulse')} />
{/snippet}

<Collapsible.Root data-slot="tool-row" {open} {onOpenChange} class={cn('flex min-w-0 flex-col', className)} {...rest}>
  {#if expandable}
    <Collapsible.Trigger
      class="flex h-7 items-center gap-2 px-1.5 text-sm hover:bg-surface focus-visible:shadow-focus-ring focus-visible:outline-none"
      >{@render row()}</Collapsible.Trigger
    >
    <Collapsible.Content class="ml-5 flex flex-col gap-2 border-l border-edge py-1.5 pl-3">
      {#each content as part}
        {#if part.type === 'text'}
          <pre
            class="max-h-64 overflow-auto rounded-md bg-surface p-2 font-mono text-xs whitespace-pre-wrap text-fg">{part.text}</pre>
        {:else if part.type === 'diff'}
          <ToolDiff
            path={part.path}
            oldText={part.old_text}
            newText={part.new_text}
            {folderPath}
            onOpen={() => onOpenLocation(part.path)}
          />
        {/if}
      {/each}
      {#each locations as location}
        <button
          type="button"
          class="self-start text-left font-mono text-xs text-accent hover:underline focus-visible:shadow-focus-ring focus-visible:outline-none"
          onclick={() => onOpenLocation(location.path, location.line ?? undefined)}
          >{location.path}{location.line != null ? `:${location.line}` : ''}</button
        >
      {/each}
    </Collapsible.Content>
  {:else}
    <div class="flex h-7 items-center gap-2 px-1.5 text-sm hover:bg-surface">{@render row()}</div>
  {/if}
  {#if children}{@render children()}{/if}
</Collapsible.Root>
