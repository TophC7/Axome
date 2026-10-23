<script lang="ts">
  import type { ComponentProps } from 'svelte'
  import * as Collapsible from '$lib/components/ui/collapsible'
  import { statusToneText } from '$lib/components/ui/status-dot'
  import { ChevronRight } from '$lib/icons/lucideExports'
  import type { ToolStep } from '$lib/features/chat/turns'
  import { cn } from '$lib/utils/cn'
  import { kindLabel, toolPreview, toolStatusTone } from './kinds'
  import ToolDiff from './ToolDiff.svelte'
  import ToolFields from './ToolFields.svelte'

  let {
    call,
    open,
    onOpenChange,
    onOpenLocation,
    folderPath,
    class: className,
    ...rest
  }: Omit<ComponentProps<typeof Collapsible.Root>, 'child' | 'children'> & {
    call: ToolStep
    open: boolean
    onOpenChange: (open: boolean) => void
    onOpenLocation: (path: string, line?: number) => void
    folderPath: string
  } = $props()
  const preview = $derived(toolPreview(call, folderPath))
  const state = $derived(toolStatusTone(call))
  const path = $derived(
    call.arguments !== null &&
      typeof call.arguments === 'object' &&
      'path' in call.arguments &&
      typeof call.arguments.path === 'string'
      ? call.arguments.path
      : null
  )
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
    <span class={cn('shrink-0 font-mono text-xs font-semibold', statusToneText[state.tone])}
      >{kindLabel(call.name)}</span
    >
    <span class="min-w-0 flex-1 truncate font-mono text-xs text-muted" title={preview}>{preview}</span>
    <span class="shrink-0 text-xs text-subtle">{state.label}</span>
    <ChevronRight
      size={12}
      aria-hidden="true"
      class={cn('duration-fast shrink-0 text-muted transition-transform ease-out-quart', open && 'rotate-90')}
    />
  </Collapsible.Trigger>
  <Collapsible.Content class="flex min-w-0 flex-col gap-2 border-t border-edge p-2.5">
    <ToolFields {call} />
    {#if call.name === 'edit' || call.name === 'write'}
      <ToolDiff {call} {folderPath} onOpen={() => path && onOpenLocation(path)} />
    {/if}
    {#each call.output as part}
      {#if part.type === 'text'}
        <pre
          class={cn(
            'max-h-96 overflow-auto rounded-md bg-ground p-2 text-xs whitespace-pre-wrap text-fg',
            call.name === 'bash' ? 'font-mono' : 'font-sans'
          )}>{part.text}</pre>
      {:else if part.type === 'image'}
        <img
          class="max-h-96 max-w-full object-contain"
          src={`data:${part.mimeType};base64,${part.data}`}
          alt={`${call.name} result`}
        />
      {/if}
    {/each}
    {#if path && call.name !== 'edit' && call.name !== 'write'}
      <button
        type="button"
        class="self-start text-left font-mono text-xs break-all text-accent hover:underline focus-visible:shadow-focus-ring focus-visible:outline-none"
        onclick={() => path && onOpenLocation(path)}>{path}</button
      >
    {/if}
  </Collapsible.Content>
</Collapsible.Root>
