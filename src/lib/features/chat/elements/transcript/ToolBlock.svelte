<script lang="ts">
  import type { ChatController } from '$lib/features/chat/controller.svelte'
  import type { ToolStep } from '$lib/features/chat/turns'
  import type { BlockView } from '$lib/features/chat/view'
  import { AskCard } from '../ask'
  import BashCommand from '../tool/BashCommand.svelte'
  import BashOutput from '../tool/BashOutput.svelte'
  import ReadOutput from '../tool/ReadOutput.svelte'
  import SearchOutput from '../tool/SearchOutput.svelte'
  import ToolDiff from '../tool/ToolDiff.svelte'
  import ToolFields from '../tool/ToolFields.svelte'
  import ToolOutput from '../tool/ToolOutput.svelte'
  import { openReferenceResult } from '../tool/referenceNavigation'
  import { toolHeader } from '../tool/toolHeader'
  import ToolFrame from './ToolFrame.svelte'

  let {
    call,
    expansion = 'collapsed',
    overrides,
    onView,
    folderPath,
    onOpenLocation,
    controller,
    nested = false
  }: {
    call: ToolStep
    expansion?: BlockView
    overrides: Record<string, BlockView>
    onView: (id: string, view: BlockView) => void
    folderPath: string
    onOpenLocation: (path: string, line?: number) => void
    controller: ChatController
    nested?: boolean
  } = $props()

  const header = $derived(toolHeader(call, folderPath))
  const activeQuestions = $derived(
    controller.state?.live.questions.find((questions) => questions.callId === call.callId) ?? null
  )
  const failed = $derived(call.status === 'error')
  // A pending question stays answerable even when the surrounding work is folded.
  // Bulk expansion opens calls; only their More control reveals unabridged output.
  const view = $derived(
    activeQuestions !== null ? 'full' : (overrides[call.id] ?? (expansion === 'collapsed' ? 'collapsed' : 'preview'))
  )
  const known = $derived(['read', 'edit', 'write', 'ask'].includes(call.name))
  const toggle = () => onView(call.id, view === 'collapsed' ? 'preview' : 'collapsed')
  const expand = () => onView(call.id, 'full')
</script>

<div data-slot="tool-block" data-collapsed={view === 'collapsed'} class="min-w-0">
  <ToolFrame {header} {view} {nested} {onOpenLocation} onToggle={toggle}>
    {#if call.name === 'ask' && activeQuestions}
      <AskCard
        questions={activeQuestions}
        onAnswer={(callId, answers) => void controller.answer(callId, answers)}
        onDismiss={(callId) => void controller.dismiss(callId)}
      />
    {:else if call.name === 'bash'}
      <div class="flex min-w-0 flex-col gap-2">
        <BashCommand command={header.command ?? ''} />
        <BashOutput {call} {view} onExpand={expand} {onOpenLocation} />
      </div>
    {:else if call.name === 'grep' || call.name === 'find'}
      <SearchOutput
        {call}
        {view}
        onExpand={expand}
        onOpenResult={(path, line) => void openReferenceResult(call, folderPath, path, line, onOpenLocation)}
      />
    {:else if call.name === 'read' && !failed}
      <ReadOutput {call} {folderPath} {view} onExpand={expand} />
    {:else}
      <div class="flex min-w-0 flex-col gap-3">
        {#if (call.name === 'edit' || call.name === 'write') && !failed}
          <ToolDiff {call} {folderPath} {view} onExpand={expand} />
        {:else}
          {#if !known}<ToolFields {call} />{/if}
          <ToolOutput parts={call.output} {view} {failed} onExpand={expand} />
        {/if}
      </div>
    {/if}
  </ToolFrame>
</div>
