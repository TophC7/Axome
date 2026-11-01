<script lang="ts">
  import { IconButton } from '$lib/components/ui/button'
  import { StatusDot } from '$lib/components/ui/status-dot'
  import type { ChatController } from '$lib/features/chat/controller.svelte'
  import * as Message from '$lib/features/chat/elements/message'
  import { formatCost, formatCount, formatDuration, formatTokens } from '$lib/features/chat/format'
  import { turnDurationMs, type ToolStep, type Turn } from '$lib/features/chat/turns'
  import type { BlockView } from '$lib/features/chat/view'
  import { AlertTriangle, ChevronRight, CircleXIcon, CopyIcon, GitForkIcon, InfoIcon } from '$lib/icons/lucideExports'
  import { copyToClipboard } from '$lib/utils/clipboard'
  import { cn } from '$lib/utils/cn'
  import ThinkingBlock from './ThinkingBlock.svelte'
  import ToolBlock from './ToolBlock.svelte'
  import SummaryBlock from './SummaryBlock.svelte'
  import ToolGroup from './ToolGroup.svelte'
  import { groupSteps, splitWork, type TranscriptStep } from './groupSteps'

  let {
    turn,
    controller,
    folderPath,
    canBranch,
    now,
    expansion = 'collapsed',
    overrides,
    onView,
    onOpenLocation
  }: {
    turn: Turn
    controller: ChatController
    folderPath: string
    canBranch: boolean
    now: number
    expansion?: BlockView
    overrides: Record<string, BlockView>
    onView: (stepId: string, view: BlockView) => void
    onOpenLocation: (path: string, line?: number) => void
  } = $props()

  const viewId = $derived(`turn:${turn.id}`)
  const open = $derived((overrides[viewId] ?? expansion) !== 'collapsed')
  const detailView = $derived(expansion === 'full' ? 'full' : 'collapsed')
  const lastAssistant = $derived(turn.assistants.at(-1))
  const duration = $derived(turnDurationMs(turn, now))
  const split = $derived(splitWork(groupSteps(turn.steps), turn.running))
  const work = $derived(split.work)
  const answer = $derived(split.answer)
  const asking = $derived(new Set(controller.state?.live.questions.map((questions) => questions.callId)))
  // Folded work still shows what needs the user (a pending question) and, while running, what the agent says.
  const shown = $derived(
    open
      ? work
      : work.filter((step) =>
          step.kind === 'text'
            ? turn.running
            : step.kind === 'tool'
              ? asking.has(step.callId)
              : step.kind === 'toolGroup'
                ? step.calls.some((call) => asking.has(call.callId))
                : step.kind !== 'thinking'
        )
  )
  const calls = $derived(turn.steps.filter((step): step is ToolStep => step.kind === 'tool'))
  const failed = $derived(calls.filter((call) => call.status === 'error').length)
  const thoughts = $derived(work.filter((step) => step.kind === 'thinking').length)
  const summary = $derived(
    [
      turn.running ? 'Working' : '',
      turn.running && duration !== null ? formatDuration(duration) : '',
      calls.length ? formatCount(calls.length, 'tool call') : '',
      thoughts ? formatCount(thoughts, 'thought') : ''
    ]
      .filter(Boolean)
      .join(' · ')
  )
  // Copy the reply; a turn stopped before answering still copies whatever it said.
  const copyText = $derived(
    (answer.some((step) => step.kind === 'text') ? answer : work)
      .flatMap((step) => (step.kind === 'text' ? [step.text] : []))
      .join('\n\n')
  )
  const usage = $derived.by(() => {
    if (!turn.assistants.length) return ''
    const sum = { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, cost: 0 }
    for (const entry of turn.assistants) {
      sum.input += entry.body.usage.input
      sum.output += entry.body.usage.output
      sum.cacheRead += entry.body.usage.cacheRead
      sum.cacheWrite += entry.body.usage.cacheWrite
      sum.cost += entry.body.usage.cost
    }
    return [
      `${formatTokens(sum.input)} in`,
      `${formatTokens(sum.output)} out`,
      sum.cacheRead ? `${formatTokens(sum.cacheRead)} cache R` : '',
      sum.cacheWrite ? `${formatTokens(sum.cacheWrite)} cache W` : '',
      formatCost(sum.cost)
    ]
      .filter(Boolean)
      .join(' · ')
  })
  const notices = $derived.by(() => {
    const rows: { text: string; danger: boolean }[] = []
    for (const entry of turn.assistants) {
      const body = entry.body
      if (body.stopReason === 'error') rows.push({ text: body.error || 'The response failed.', danger: true })
      else if (body.stopReason === 'length') rows.push({ text: 'Stopped at the output token limit.', danger: false })
      else if (body.stopReason === 'aborted') rows.push({ text: 'The response was aborted.', danger: false })
    }
    const end = turn.end
    if (end && end.outcome !== 'done') {
      const fallback = {
        aborted: 'The turn was aborted.',
        failed: 'The turn failed.',
        interrupted: 'The turn was interrupted.'
      }
      const text = end.reason || fallback[end.outcome]
      if (!rows.some((row) => row.text === text)) rows.push({ text, danger: end.outcome === 'failed' })
    }
    return rows
  })
</script>

{#snippet render(step: TranscriptStep)}
  {#if step.kind === 'text'}
    <Message.Response text={step.text} streaming={step.entryId === null} {folderPath} />
  {:else if step.kind === 'thinking'}
    <ThinkingBlock thought={step} view={overrides[step.id] ?? detailView} {onView} {folderPath} />
  {:else if step.kind === 'toolGroup'}
    <ToolGroup group={step} expansion={detailView} {overrides} {onView} {folderPath} {onOpenLocation} {controller} />
  {:else if step.kind === 'tool'}
    <ToolBlock call={step} expansion={detailView} {overrides} {onView} {folderPath} {onOpenLocation} {controller} />
  {:else if step.kind === 'notice'}
    <div class="text-sm whitespace-pre-wrap text-muted">{step.text}</div>
  {:else if step.kind === 'compaction' || step.kind === 'branchSummary'}
    <SummaryBlock {step} view={overrides[step.id] ?? detailView} {onView} {folderPath} />
  {/if}
{/snippet}

<div data-slot="agent-block" class="flex min-w-0 flex-col gap-3 py-2">
  {#if turn.running || work.length}
    <button
      type="button"
      aria-expanded={open}
      disabled={!work.length}
      onclick={() => onView(viewId, open ? 'collapsed' : 'preview')}
      class="flex items-center gap-2 self-start rounded text-sm text-muted transition-colors focus-visible:shadow-focus-ring focus-visible:outline-none enabled:cursor-pointer enabled:hover:text-fg"
    >
      {#if turn.running}<StatusDot tone="working" size="sm" />{/if}
      <span class="tabular-nums">{summary}</span>
      {#if failed}<span class="text-danger">{failed} failed</span>{/if}
      {#if work.length}<ChevronRight size={14} class={cn('transition-transform', open && 'rotate-90')} />{/if}
    </button>
  {/if}
  {#if shown.length}
    <div class={cn('flex min-w-0 flex-col gap-3', open && 'border-l border-edge pl-3')}>
      {#each shown as step (step.id)}{@render render(step)}{/each}
    </div>
  {/if}
  {#each answer as step (step.id)}{@render render(step)}{/each}
  {#each notices as notice}
    {@const Icon = notice.danger ? CircleXIcon : AlertTriangle}
    <div class={cn('flex items-start gap-2 text-sm', notice.danger ? 'text-danger' : 'text-warning')}>
      <Icon size={14} class="shrink-0" />
      <span class="whitespace-pre-wrap">{notice.text}</span>
    </div>
  {/each}
  {#if !turn.running}
    <div class="flex min-h-5 max-w-full items-center gap-1 self-start text-subtle">
      {#if lastAssistant}
        <IconButton tooltip="Copy" class="shrink-0 text-subtle" onclick={() => void copyToClipboard(copyText)}
          ><CopyIcon size={14} /></IconButton
        >
        <IconButton
          tooltip="Fork"
          class="shrink-0 text-subtle"
          disabled={!canBranch}
          onclick={() => void controller.fork(lastAssistant.id)}><GitForkIcon size={14} /></IconButton
        >
      {/if}
      {#if duration !== null || usage}
        <IconButton
          ariaLabel="Response Details"
          tooltip={[duration === null ? null : formatDuration(duration), usage].filter(Boolean).join(' · ')}
          tooltipSide="top"
          class="shrink-0 text-subtle"
        >
          <InfoIcon size={14} />
        </IconButton>
      {/if}
      {#if lastAssistant}
        <span class="ml-1 truncate font-mono text-xs text-muted" title={lastAssistant.body.model}
          >{lastAssistant.body.model}</span
        >
      {/if}
    </div>
  {/if}
</div>
