<script lang="ts">
  import type { HTMLButtonAttributes } from 'svelte/elements'
  import type { ChatTurn } from '$lib/types/backend'
  import { Button } from '$lib/components/ui/button'
  import * as Tooltip from '$lib/components/ui/tooltip'
  import { ArrowUp, LoaderCircle, SquareIcon } from '$lib/icons/lucideExports'
  import { cn } from '$lib/utils/cn'

  let {
    turn,
    canSend,
    onSend,
    onStop,
    class: className,
    ...rest
  }: Omit<HTMLButtonAttributes, 'children' | 'disabled' | 'onclick'> & {
    turn: ChatTurn
    canSend: boolean
    onSend: () => void
    onStop: () => void
  } = $props()
  const label = $derived(
    turn === 'idle' ? 'Send' : turn === 'running' ? 'Stop' : turn === 'starting' ? 'Starting' : 'Reconnecting'
  )
</script>

<Tooltip.Root>
  <Tooltip.Trigger>
    {#snippet child({ props })}
      <Button
        {...props}
        data-slot="prompt-input-submit"
        size="icon"
        variant={turn === 'idle' ? 'accent' : 'outline'}
        aria-label={label}
        disabled={turn === 'idle' ? !canSend : turn !== 'running'}
        onclick={turn === 'running' ? onStop : onSend}
        class={cn(className)}
        {...rest}
      >
        {#if turn === 'idle'}<ArrowUp size={14} />
        {:else if turn === 'running'}<SquareIcon size={10} class="fill-current" />
        {:else}<LoaderCircle size={14} class="animate-spin" />{/if}
      </Button>
    {/snippet}
  </Tooltip.Trigger>
  <Tooltip.Content side="top">{label}</Tooltip.Content>
</Tooltip.Root>
