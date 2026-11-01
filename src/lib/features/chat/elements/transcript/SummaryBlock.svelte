<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import * as Message from '$lib/features/chat/elements/message'
  import { formatTokens } from '$lib/features/chat/format'
  import type { Step } from '$lib/features/chat/turns'
  import type { BlockView } from '$lib/features/chat/view'

  let {
    step,
    view,
    onView,
    folderPath
  }: {
    step: Extract<Step, { kind: 'compaction' | 'branchSummary' }>
    view: BlockView
    onView: (stepId: string, view: BlockView) => void
    folderPath: string
  } = $props()
</script>

<div data-slot="summary-block" class="min-w-0">
  <Button
    type="button"
    variant="text"
    size="xs"
    aria-expanded={view !== 'collapsed'}
    class="flex w-full gap-2 px-0 py-2 font-mono text-xs text-subtle transition-none"
    onclick={() => onView(step.id, view === 'collapsed' ? 'preview' : 'collapsed')}
  >
    <span class="h-px flex-1 bg-edge"></span>
    <span
      >{step.kind === 'compaction' ? `compacted · ${formatTokens(step.tokensBefore)} tokens` : 'branch summary'}</span
    >
    <span class="h-px flex-1 bg-edge"></span>
  </Button>
  {#if view !== 'collapsed'}
    <Message.Response text={step.summary} streaming={false} {folderPath} />
  {/if}
</div>
