<script lang="ts">
  import type { ComponentProps, Snippet } from 'svelte'
  import * as Collapsible from '$lib/components/ui/collapsible'
  import { statusToneText } from '$lib/components/ui/status-dot'
  import { ChevronRight } from '$lib/icons/lucideExports'
  import { cn } from '$lib/utils/cn'
  import { kindLabel, toolPreview, toolStatusTone, type ToolEntry } from './kinds'
  import ToolDiff from './ToolDiff.svelte'
  import ToolFields from './ToolFields.svelte'

  let {
    entry,
    awaiting,
    open,
    onOpenChange,
    onOpenLocation,
    folderPath,
    children,
    class: className,
    ...rest
  }: Omit<ComponentProps<typeof Collapsible.Root>, 'child' | 'children'> & {
    entry: ToolEntry
    awaiting: boolean
    open: boolean
    onOpenChange: (open: boolean) => void
    onOpenLocation: (path: string, line?: number) => void
    folderPath: string
    children?: Snippet
  } = $props()
  const label = $derived(entry.name ?? kindLabel(entry.tool_kind))
  const preview = $derived(toolPreview(entry, folderPath))
  const state = $derived(toolStatusTone(entry.status, awaiting))
</script>

<Collapsible.Root
  data-slot="tool-row"
  {open}
  {onOpenChange}
  class={cn('flex min-w-0 flex-col rounded-md border border-edge bg-surface', className)}
  {...rest}
>
  <Collapsible.Trigger
    class="flex min-h-7 min-w-0 items-center gap-2 rounded-md px-2.5 py-1.5 text-left hover:bg-raised focus-visible:shadow-focus-ring focus-visible:outline-none"
  >
    <span class={cn('shrink-0 font-mono text-xs font-semibold', statusToneText[state.tone])}>{label}</span>
    <span class="min-w-0 flex-1 truncate font-mono text-xs text-muted" title={preview}>{preview}</span>
    <span class="shrink-0 text-xs text-subtle">{state.label}</span>
    <ChevronRight
      size={12}
      aria-hidden="true"
      class={cn('duration-fast shrink-0 text-muted transition-transform ease-out-quart', open && 'rotate-90')}
    />
  </Collapsible.Trigger>
  <Collapsible.Content class="flex min-w-0 flex-col gap-2 border-t border-edge p-2.5">
    <ToolFields input={entry.input} />
    {#each entry.content as part}
      {#if part.type === 'text'}
        <pre
          class="max-h-96 overflow-auto rounded-md bg-ground p-2 font-mono text-xs whitespace-pre-wrap text-fg">{part.text}</pre>
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
    {#each entry.locations as location}
      <button
        type="button"
        class="self-start text-left font-mono text-xs break-all text-accent hover:underline focus-visible:shadow-focus-ring focus-visible:outline-none"
        onclick={() => onOpenLocation(location.path, location.line ?? undefined)}
        >{location.path}{location.line != null ? `:${location.line}` : ''}</button
      >
    {/each}
  </Collapsible.Content>
  {#if children}{@render children()}{/if}
</Collapsible.Root>
