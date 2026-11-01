<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import { formatCount } from '$lib/features/chat/format'
  import type { ToolStep } from '$lib/features/chat/turns'
  import type { BlockView } from '$lib/features/chat/view'
  import MoreButton from './MoreButton.svelte'
  import { searchOutput } from './searchOutput'
  import TerminalOutput from './TerminalOutput.svelte'
  import ToolImage from './ToolImage.svelte'

  let {
    call,
    view,
    onExpand,
    onOpenResult
  }: {
    call: ToolStep
    view: BlockView
    onExpand: () => void
    onOpenResult: (path: string, line?: number) => void
  } = $props()

  const output = $derived(searchOutput(call, view === 'preview' ? 12 : null))
  const hasContent = $derived(
    output.parts.some((part) => part.type === 'image' || part.lines.length > 0 || part.suffix.length > 0)
  )
  const running = $derived(call.status === 'running')
</script>

<TerminalOutput output={call.output}>
  <div data-slot="search-output" class="min-w-0 font-mono text-xs text-fg">
    {#if hasContent}
      {#each output.parts as part}
        {#if part.type === 'text'}
          <pre
            class="m-0 font-mono text-xs wrap-anywhere whitespace-pre-wrap">{#each part.lines as row}{#if row.link}<Button
                  type="button"
                  variant="text"
                  size="xs"
                  class="inline p-0 align-baseline font-mono text-xs font-normal wrap-anywhere whitespace-pre-wrap text-muted underline decoration-subtle underline-offset-2 select-text hover:text-bright"
                  onclick={() => onOpenResult(row.link!.path, row.link!.line)}
                  >{row.text.slice(0, row.link.length)}</Button
                >{row.text.slice(row.link.length)}{:else}{row.text}{/if}{/each}{part.suffix}</pre>
        {:else}
          <ToolImage {part} />
        {/if}
      {/each}
    {:else}
      <div class="text-subtle">{running ? 'Running · No output yet' : 'No output'}</div>
    {/if}
  </div>
  {#if output.hidden > 0}
    <MoreButton label={formatCount(output.hidden, 'more line')} {onExpand} />
  {/if}
</TerminalOutput>
