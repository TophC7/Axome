<script lang="ts">
  import { contentLines, previewDiff, sourceRows } from '$lib/features/chat/lineDiff'
  import type { ToolStep } from '$lib/features/chat/turns'
  import type { BlockView } from '$lib/features/chat/view'
  import { wordHighlights } from './diffHighlight'
  import { editDiff, record } from './payload'
  import ToolCode from './ToolCode.svelte'
  import ToolOutput from './ToolOutput.svelte'

  let {
    call,
    folderPath,
    view,
    onExpand
  }: {
    call: ToolStep
    folderPath: string
    view: BlockView
    onExpand: () => void
  } = $props()

  const limit = $derived(view === 'preview' ? 40 : null)
  const isEdit = $derived(call.name === 'edit')
  const args = $derived(record(call.arguments))
  const rows = $derived(
    isEdit
      ? (editDiff(call.result?.details)?.rows ?? null)
      : typeof args.content === 'string'
        ? sourceRows(contentLines(args.content), 1)
        : null
  )
  // Hunk budgeting only applies to changes; written content is plain source that ToolCode slices.
  const preview = $derived(isEdit && limit !== null ? previewDiff(rows ?? [], limit, 8) : undefined)
  const emphasis = $derived(isEdit ? wordHighlights(rows ?? []) : undefined)
</script>

<div data-slot="tool-diff" class="min-w-0 font-mono text-xs">
  {#if rows}
    {#if rows.length}
      <ToolCode
        {rows}
        path={typeof args.path === 'string' ? args.path : ''}
        {folderPath}
        {limit}
        {onExpand}
        {preview}
        {emphasis}
        neutral={!isEdit}
      />
    {:else}
      <div class="text-subtle">No changes</div>
    {/if}
  {:else if call.output.some((part) => part.type === 'image' || part.text.trim())}
    <ToolOutput parts={call.output} {view} {onExpand} />
  {:else}
    <div class="text-subtle">{call.result ? 'No diff recorded' : 'Waiting for result'}</div>
  {/if}
</div>
