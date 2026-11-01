<script lang="ts">
  import { sourceRows } from '$lib/features/chat/lineDiff'
  import type { ToolStep } from '$lib/features/chat/turns'
  import type { BlockView } from '$lib/features/chat/view'
  import ToolCode from './ToolCode.svelte'
  import ToolOutput from './ToolOutput.svelte'
  import { readOutput } from './readOutput'

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
  const source = $derived(readOutput(call))
  // readOutput counts a trailing empty line as source, so keep it rather than using contentLines.
  const rows = $derived(source && source.lineCount > 0 ? sourceRows(source.body.split('\n'), source.startLine) : [])
</script>

<div data-slot="read-output" class="min-w-0 font-mono text-xs">
  {#if source}
    {#if rows.length}
      <ToolCode {rows} path={source.path} {folderPath} {limit} {onExpand} neutral />
    {:else}
      <div class="text-subtle">No source text recorded</div>
    {/if}
    {#if source.notice}
      <pre class="mt-1 font-mono wrap-anywhere whitespace-pre-wrap text-subtle">{source.notice}</pre>
    {/if}
    {#if source.attachments.length}
      <div class="mt-2">
        <ToolOutput parts={source.attachments} view="full" {onExpand} />
      </div>
    {/if}
  {:else}
    <ToolOutput parts={call.output} {view} {onExpand} />
  {/if}
</div>
