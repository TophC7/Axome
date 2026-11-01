<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import { formatCount } from '$lib/features/chat/format'
  import type { ToolStep } from '$lib/features/chat/turns'
  import type { BlockView } from '$lib/features/chat/view'
  import { bashOutput } from './bashOutput'
  import MoreButton from './MoreButton.svelte'
  import TerminalOutput from './TerminalOutput.svelte'
  import ToolImage from './ToolImage.svelte'

  let {
    call,
    view,
    onExpand,
    onOpenLocation
  }: {
    call: ToolStep
    view: BlockView
    onExpand: () => void
    onOpenLocation: (path: string, line?: number) => void
  } = $props()

  const output = $derived(bashOutput(call, view === 'preview' ? 12 : null))
  const hasContent = $derived(output.parts.some((part) => part.type === 'image' || part.text.length > 0))
  const running = $derived(call.status === 'running')
  const noticeSummary = $derived(
    output.notice?.replace(/^Showing lines /, 'Recorded lines ') ?? 'Recorded output is truncated.'
  )
</script>

<TerminalOutput output={call.output}>
  <div data-slot="bash-output" class="contents">
    {#if output.hidden > 0}
      <MoreButton label={formatCount(output.hidden, 'earlier line')} {onExpand} />
    {/if}
    {#if hasContent}
      <div class="min-w-0 font-mono text-xs text-fg">
        {#each output.parts as part}
          {#if part.type === 'text' && part.text}
            <pre class="m-0 font-mono text-xs wrap-anywhere whitespace-pre-wrap">{part.text}</pre>
          {:else if part.type === 'image'}
            <ToolImage {part} />
          {/if}
        {/each}
      </div>
    {:else}
      <div class="font-mono text-xs text-subtle">{running ? 'Running · No output yet' : 'No output'}</div>
    {/if}
    {#if output.fullOutputPath}
      <div class="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-subtle">
        <span>{noticeSummary}</span>
        <Button
          type="button"
          variant="text"
          size="xs"
          class="px-0 font-normal"
          title={output.fullOutputPath}
          onclick={() => onOpenLocation(output.fullOutputPath!)}>Open Full Output</Button
        >
      </div>
    {/if}
    {#if output.status}
      <pre class="m-0 font-mono text-xs wrap-anywhere whitespace-pre-wrap text-danger-bright">{output.status}</pre>
    {/if}
  </div>
</TerminalOutput>
