<script lang="ts">
  import type { ChatController } from '$lib/features/chat/controller.svelte'
  import { formatCount } from '$lib/features/chat/format'
  import type { BlockView } from '$lib/features/chat/view'
  import type { ToolHeader } from '../tool/toolHeader'
  import type { ToolGroupStep } from './groupSteps'
  import ToolBlock from './ToolBlock.svelte'
  import ToolFrame from './ToolFrame.svelte'

  let {
    group,
    expansion = 'collapsed',
    overrides,
    onView,
    folderPath,
    onOpenLocation,
    controller
  }: {
    group: ToolGroupStep
    expansion?: BlockView
    overrides: Record<string, BlockView>
    onView: (id: string, view: BlockView) => void
    folderPath: string
    onOpenLocation: (path: string, line?: number) => void
    controller: ChatController
  } = $props()

  // Group folding must not overwrite the first call's individual view.
  const viewId = $derived(`tool-group:${group.id}`)
  const failed = $derived(group.calls.filter((call) => call.status === 'error').length)
  const running = $derived(group.calls.filter((call) => call.status === 'running').length)
  const pending = $derived(
    controller.state?.live.questions.some((questions) =>
      group.calls.some((call) => call.callId === questions.callId)
    ) ?? false
  )
  const view = $derived(pending ? 'full' : (overrides[viewId] ?? expansion))
  // The batch never takes the danger tone, so a failed nested call keeps its own visible tint.
  const header: ToolHeader = $derived({
    name: group.calls[0].name,
    summary: formatCount(group.calls.length, 'call'),
    path: null,
    line: undefined,
    pathLabel: '',
    command: null,
    lines: null,
    meta: '',
    tone: running ? 'working' : 'success',
    status: failed
      ? { label: [`${failed} failed`, running ? `${running} running` : ''].filter(Boolean).join(' · '), tone: 'danger' }
      : running
        ? { label: `${running} running`, tone: 'working' }
        : null
  })
</script>

<div data-slot="tool-group" data-collapsed={view === 'collapsed'} class="min-w-0">
  <ToolFrame
    {header}
    {view}
    {onOpenLocation}
    onToggle={() => onView(viewId, view === 'collapsed' ? 'preview' : 'collapsed')}
  >
    <div class="flex min-w-0 flex-col border-l border-edge">
      {#each group.calls as call (call.id)}
        <ToolBlock {call} {expansion} {overrides} {onView} {folderPath} {onOpenLocation} {controller} nested />
      {/each}
    </div>
  </ToolFrame>
</div>
