<script lang="ts">
  import type { Snippet } from 'svelte'
  import { Button } from '$lib/components/ui/button'
  import { NetLineChange } from '$lib/components/ui/net-line-change'
  import type { BlockView } from '$lib/features/chat/view'
  import {
    CircleXIcon,
    FileCodeIcon,
    FileDiff,
    FilePlusCornerIcon,
    FolderOpenIcon,
    MessageSquare,
    PackageIcon,
    SearchIcon,
    TerminalIcon
  } from '$lib/icons/lucideExports'
  import FileIcon from '$lib/icons/FileIcon.svelte'
  import { cn } from '$lib/utils/cn'
  import type { ToolHeader } from '../tool/toolHeader'
  import BashCommand from '../tool/BashCommand.svelte'

  let {
    header,
    view,
    nested = false,
    onToggle,
    onOpenLocation,
    children
  }: {
    header: ToolHeader
    view: BlockView
    nested?: boolean
    onToggle: () => void
    onOpenLocation: (path: string, line?: number) => void
    children?: Snippet
  } = $props()

  const tools: Record<string, { icon: typeof TerminalIcon; label: string }> = {
    bash: { icon: TerminalIcon, label: 'Bash' },
    read: { icon: FileCodeIcon, label: 'Read' },
    edit: { icon: FileDiff, label: 'Edit' },
    write: { icon: FilePlusCornerIcon, label: 'Write' },
    grep: { icon: SearchIcon, label: 'Grep' },
    find: { icon: FolderOpenIcon, label: 'Find' },
    ask: { icon: MessageSquare, label: 'Ask' }
  }
  const tool = $derived(tools[header.name] ?? { icon: PackageIcon, label: header.name })
  const ToolIcon = $derived(tool.icon)
  const failed = $derived(header.tone === 'danger')
  const running = $derived(header.tone === 'working')

  function toggleFromContent(event: MouseEvent) {
    const target = event.target
    if (
      window.getSelection()?.toString() ||
      !(target instanceof Element) ||
      target.closest('a, button, input, textarea, select, label, form, [contenteditable], [role="button"]') ||
      // A nested call owns its own clicks; its batch only toggles from its header or the gaps between calls.
      target.closest('[data-slot="tool-frame"]') !== event.currentTarget
    ) {
      return
    }
    onToggle()
  }
</script>

<!-- The tool-name button provides keyboard access; the surrounding output also accepts an unselected click. -->
<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<div
  data-slot="tool-frame"
  class={cn(
    'flex min-w-0 cursor-pointer flex-col gap-1.5 font-mono text-xs',
    nested ? 'px-2 py-1.5' : 'px-3 py-2',
    failed ? 'bg-danger-bg' : nested ? '' : 'bg-surface',
    // Only the innermost hovered frame tints, so a batch stays still while a nested call is hovered.
    failed
      ? '[&:hover:not(:has([data-slot=tool-frame]:hover))]:bg-danger/20'
      : '[&:hover:not(:has([data-slot=tool-frame]:hover))]:bg-raised/60'
  )}
  onclick={toggleFromContent}
>
  <div class="flex min-w-0 flex-wrap items-start gap-x-2 gap-y-1">
    <!-- The batch header already names the tool; nested calls keep the toggle for keyboard users only. -->
    {#if !nested}
      <ToolIcon
        size={14}
        class={cn('mt-px shrink-0', failed ? 'text-danger' : running ? 'text-accent' : 'text-muted')}
        aria-hidden="true"
      />
    {/if}
    <Button
      variant="text"
      size="xs"
      class={cn(
        'shrink-0 p-0 font-mono text-xs font-medium hover:text-bright',
        failed ? 'text-danger-bright' : running ? 'text-accent' : 'text-fg',
        nested && 'sr-only focus-visible:not-sr-only'
      )}
      aria-expanded={view !== 'collapsed'}
      aria-label={`${view === 'collapsed' ? 'Expand' : 'Collapse'} ${header.name} output`}
      onclick={onToggle}>{tool.label}</Button
    >
    <div class="flex min-w-32 flex-1 items-start gap-2">
      {#if header.command !== null && view === 'collapsed'}
        <BashCommand command={header.command} inline />
      {:else if header.summary && header.command === null}
        <span class="min-w-0 truncate text-fg">{header.summary}</span>
      {/if}
      {#if header.path !== null}
        {@const search = header.name === 'grep' || header.name === 'find'}
        {#if search}<span class="text-subtle">in</span>{/if}
        <Button
          variant="text"
          size="xs"
          class="min-w-0 justify-start p-0 font-mono text-xs font-normal text-fg hover:text-bright hover:underline"
          onclick={() => header.path !== null && onOpenLocation(header.path, header.line)}
          >{#if !search}<FileIcon filename={header.path} size={12} />{/if}<span class="truncate"
            >{header.pathLabel || header.path}</span
          ></Button
        >
      {/if}
    </div>
    <div class="ms-auto flex shrink-0 items-center gap-2">
      {#if header.lines}<NetLineChange added={header.lines.added} removed={header.lines.removed} />{/if}
      {#if header.meta}<span class="text-subtle">{header.meta}</span>{/if}
      {#if header.status}
        <span
          class={cn(
            'inline-flex items-center gap-1',
            header.status.tone === 'danger' ? 'text-danger-bright' : 'text-accent'
          )}
        >
          {#if header.status.tone === 'danger'}<CircleXIcon size={12} aria-hidden="true" />{/if}
          {header.status.label}
        </span>
      {/if}
    </div>
  </div>
  {#if view !== 'collapsed' && children}
    <div class="min-w-0">
      {@render children()}
    </div>
  {/if}
</div>
