<script lang="ts">
  import { Button, IconButton } from '$lib/components/ui/button'
  import { Textarea } from '$lib/components/ui/input'
  import type { ChatController } from '$lib/features/chat/controller.svelte'
  import type { TurnConfig, UserEntry } from '$lib/features/chat/turns'
  import { ChevronLeft, ChevronRight, CopyIcon, GitForkIcon, PencilIcon } from '$lib/icons/lucideExports'
  import { copyToClipboard } from '$lib/utils/clipboard'
  import { cn } from '$lib/utils/cn'
  import { KeyHint } from '../key-hint'

  let {
    entry,
    config,
    controller,
    canBranch
  }: {
    entry: UserEntry
    config: TurnConfig
    controller: ChatController
    canBranch: boolean
  } = $props()

  let textarea = $state<HTMLTextAreaElement | null>(null)
  const editing = $derived(controller.editing?.entryId === entry.id)
  const branchIndex = $derived(entry.body.siblings.indexOf(entry.id))
  const time = $derived(
    new Date(entry.timestamp).toLocaleTimeString([], {
      hour12: false,
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit'
    })
  )

  $effect(() => {
    if (editing) textarea?.focus()
  })

  $effect(() => {
    if (!editing || !textarea) return
    controller.editing?.text
    textarea.style.height = 'auto'
    textarea.style.height = `${textarea.scrollHeight}px`
  })

  function editKey(event: KeyboardEvent): void {
    if (event.isComposing) return
    if (event.key === 'Escape') {
      event.preventDefault()
      event.stopPropagation()
      controller.cancelEdit()
    } else if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault()
      event.stopPropagation()
      if (canBranch) void controller.submitEdit()
    }
  }
</script>

<div data-slot="user-block" class="flex min-w-0 flex-col items-end gap-1">
  <div class={cn('rounded-lg bg-surface px-4 py-3', editing ? 'flex w-full flex-col gap-2' : 'max-w-4/5 min-w-0')}>
    {#if editing && controller.editing}
      <Textarea
        bind:ref={textarea}
        bind:value={controller.editing.text}
        rows={1}
        aria-label="Edit Message"
        disabled={!canBranch}
        onkeydown={editKey}
        class="overflow-hidden bg-ground text-base"
      />
      <div class="flex justify-end gap-2">
        <Button type="button" variant="ghost" size="xs" onclick={() => controller.cancelEdit()}
          >Cancel <KeyHint keys="Esc" /></Button
        >
        <Button
          type="button"
          variant="ghost"
          size="xs"
          disabled={!canBranch || !controller.editing.text.trim()}
          onclick={() => void controller.submitEdit()}>Send <KeyHint keys="Enter" /></Button
        >
      </div>
    {:else}
      <div class="text-base break-words whitespace-pre-wrap text-fg">{entry.body.text}</div>
    {/if}
  </div>
  <div class="flex min-h-5 flex-wrap items-center justify-end gap-2 text-xs text-subtle">
    <time datetime={entry.timestamp} class="font-mono tabular-nums">{time}</time>
    {#if config.model}<span class="font-mono">model <span class="text-muted">{config.model}</span></span>{/if}
    {#if config.thinking}<span class="font-mono">thinking <span class="text-muted">{config.thinking}</span></span>{/if}
    {#if entry.body.siblings.length > 1}
      <div class="flex items-center gap-1 font-mono" aria-label="Message branches">
        <IconButton
          tooltip="Previous Branch"
          disabled={!canBranch || branchIndex <= 0}
          onclick={() => void controller.selectBranch(entry.body.siblings[branchIndex - 1])}
        >
          <ChevronLeft size={12} />
        </IconButton>
        <span class="tabular-nums">{branchIndex + 1}/{entry.body.siblings.length}</span>
        <IconButton
          tooltip="Next Branch"
          disabled={!canBranch || branchIndex >= entry.body.siblings.length - 1}
          onclick={() => void controller.selectBranch(entry.body.siblings[branchIndex + 1])}
        >
          <ChevronRight size={12} />
        </IconButton>
      </div>
    {/if}
    <div class="flex items-center gap-1">
      <IconButton tooltip="Edit" class="text-subtle" disabled={!canBranch} onclick={() => controller.startEdit(entry)}
        ><PencilIcon size={14} /></IconButton
      >
      <IconButton
        tooltip="Fork"
        class="text-subtle"
        disabled={!canBranch}
        onclick={() => void controller.fork(entry.id)}><GitForkIcon size={14} /></IconButton
      >
      <IconButton tooltip="Copy" class="text-subtle" onclick={() => void copyToClipboard(entry.body.text)}
        ><CopyIcon size={14} /></IconButton
      >
    </div>
  </div>
</div>
