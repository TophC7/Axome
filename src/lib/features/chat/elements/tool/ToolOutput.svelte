<script lang="ts">
  import { formatCount } from '$lib/features/chat/format'
  import type { BlockView } from '$lib/features/chat/view'
  import type { ChatContent } from '$lib/types/backend'
  import { cn } from '$lib/utils/cn'
  import MoreButton from './MoreButton.svelte'
  import TerminalOutput from './TerminalOutput.svelte'
  import ToolImage from './ToolImage.svelte'

  let {
    parts,
    view,
    onExpand,
    failed = false
  }: {
    parts: ChatContent[]
    view: BlockView
    onExpand: () => void
    failed?: boolean
  } = $props()

  const output = $derived.by(() => {
    if (view !== 'preview') return { parts, hidden: 0 }
    const lines = parts.map((part) => (part.type === 'text' && part.text ? part.text.split('\n') : []))
    // A trailing newline ends the last line; it does not start a hidden one.
    const counts = lines.map((chunk) => chunk.length - (chunk.at(-1) === '' ? 1 : 0))
    let offset = 0
    const visible = parts.flatMap((part, index): ChatContent[] => {
      if (part.type === 'image') return [part]
      const to = Math.min(counts[index], 12 - offset)
      offset += counts[index]
      return to > 0 ? [{ type: 'text', text: lines[index].slice(0, to).join('\n') }] : []
    })
    return { parts: visible, hidden: Math.max(0, offset - 12) }
  })
</script>

<TerminalOutput output={parts}>
  <div
    data-slot="tool-output"
    class={cn('flex min-w-0 flex-col gap-1 font-mono text-xs', failed ? 'text-danger-bright' : 'text-fg')}
  >
    {#each output.parts as part}
      {#if part.type === 'text'}
        <pre class="m-0 font-mono text-xs wrap-anywhere whitespace-pre-wrap">{part.text}</pre>
      {:else if part.type === 'image'}
        <ToolImage {part} />
      {/if}
    {/each}
  </div>
  {#if output.hidden > 0}
    <MoreButton label={formatCount(output.hidden, 'more line')} {onExpand} />
  {/if}
</TerminalOutput>
