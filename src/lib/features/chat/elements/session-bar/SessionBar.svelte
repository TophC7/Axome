<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import { StatusDot, statusToneText, type ToneStatus } from '$lib/components/ui/status-dot'
  import { InfoTooltip } from '$lib/components/ui/tooltip'
  import type { ChatController } from '$lib/features/chat/controller.svelte'
  import { formatDuration } from '$lib/features/chat/format'
  import ShortcutPreview from '$lib/features/command-palette/shortcuts/ShortcutPreview.svelte'
  import { copyToClipboard } from '$lib/utils/clipboard'
  import { ContextUsage } from '../context'

  let {
    controller,
    title,
    sessionId,
    elapsed,
    onReconnect,
    onStop
  }: {
    controller: ChatController
    title: string
    sessionId: string | null
    elapsed: number | null
    onReconnect: () => void
    onStop: () => void
  } = $props()

  const shortcuts = [
    ['Enter', 'Send, or steer while working'],
    ['Alt+Enter', 'Queue a follow-up'],
    ['Shift+Enter', 'New line'],
    ['Esc', 'Stop'],
    ['Shift+Tab', 'Cycle effort'],
    ['Alt+M', 'Pick model'],
    ['Alt+O', 'Cycle folded / summaries / open calls'],
    ['PageUp', 'Scroll up'],
    ['PageDown', 'Scroll down']
  ]

  const status: ToneStatus = $derived.by(() => {
    if (controller.connection === 'closed') {
      return { tone: 'danger', text: controller.error ?? 'The axo connection closed.' }
    }
    if (controller.connection === 'reconnecting') return { tone: 'waiting', text: 'Reconnecting…' }
    if (controller.connection === 'attaching' || controller.connection === 'detached') {
      return { tone: 'waiting', text: 'Connecting…' }
    }
    const live = controller.state?.live
    if (live?.maintenance) {
      return {
        tone: 'working',
        text:
          live.maintenance === 'compact'
            ? 'Compacting…'
            : live.maintenance === 'branchSummary'
              ? 'Summarizing branch…'
              : 'Updating context…'
      }
    }
    if (live?.retry) return { tone: 'waiting', text: `Retrying (attempt ${live.retry.attempt})…` }
    if (live?.questions.length) return { tone: 'waiting', text: 'Waiting for an answer' }
    if (live?.running)
      return { tone: 'working', text: elapsed === null ? 'working' : `working ${formatDuration(elapsed)}` }
    return { tone: 'idle', text: 'idle' }
  })
</script>

<div data-slot="session-bar" class="flex h-8 shrink-0 items-center gap-2 border-b border-edge bg-surface px-3 text-xs">
  <div role="status" class="flex max-w-1/2 min-w-0 shrink-0 items-center gap-2" title={status.text}>
    <StatusDot tone={status.tone} />
    <span class={`truncate ${statusToneText[status.tone]}`}>{status.text}</span>
  </div>
  {#if controller.connection === 'closed'}<Button size="xs" onclick={onReconnect}>Reconnect</Button>{/if}
  {#if controller.connection === 'live' && controller.busy}<Button size="xs" onclick={onStop}>Stop</Button>{/if}
  <span class="shrink-0 text-subtle">·</span>
  <span class="min-w-0 flex-1 truncate text-muted" {title}>{title}</span>
  {#if sessionId}
    <Button
      variant="ghost"
      size="xs"
      class="shrink-0 font-mono text-subtle"
      aria-label="Copy Session ID"
      title={sessionId}
      onclick={() => void copyToClipboard(sessionId)}>{sessionId.slice(0, 8)}</Button
    >
  {/if}
  <div class="ms-auto flex shrink-0 items-center gap-1">
    {#if controller.state}<ContextUsage stats={controller.state.live.stats} />{/if}
    <InfoTooltip ariaLabel="Chat Shortcuts" class="hover:bg-raised" contentClass="w-80">
      <div class="space-y-2">
        <p class="font-medium text-bright">Shortcuts</p>
        <div class="grid grid-cols-[auto_1fr] items-center gap-x-3 gap-y-1">
          {#each shortcuts as [spec, action] (spec)}
            <ShortcutPreview {spec} />
            <span>{action}</span>
          {/each}
        </div>
      </div>
    </InfoTooltip>
  </div>
</div>
