<script lang="ts">
  import type { HTMLButtonAttributes } from 'svelte/elements'
  import { Button } from '$lib/components/ui/button'
  import * as Tooltip from '$lib/components/ui/tooltip'
  import { ArrowUp, LoaderCircle, SquareIcon } from '$lib/icons/lucideExports'
  import { cn } from '$lib/utils/cn'

  let {
    running,
    connecting,
    canSend,
    onSend,
    onStop,
    class: className,
    ...rest
  }: Omit<HTMLButtonAttributes, 'children' | 'disabled' | 'onclick'> & {
    running: boolean
    connecting: boolean
    canSend: boolean
    onSend: () => void
    onStop: () => void
  } = $props()
  const label = $derived(connecting ? 'Connecting' : running ? 'Stop' : 'Send')
</script>

<Tooltip.Root>
  <Tooltip.Trigger>
    {#snippet child({ props })}
      <Button
        {...props}
        data-slot="prompt-input-submit"
        size="icon"
        variant={running || connecting ? 'outline' : 'accent'}
        aria-label={label}
        disabled={connecting || (!running && !canSend)}
        onclick={running ? onStop : onSend}
        class={cn(className)}
        {...rest}
      >
        {#if connecting}<LoaderCircle size={14} class="animate-spin" />
        {:else if running}<SquareIcon size={10} class="fill-current" />
        {:else}<ArrowUp size={14} />{/if}
      </Button>
    {/snippet}
  </Tooltip.Trigger>
  <Tooltip.Content side="top">{label}</Tooltip.Content>
</Tooltip.Root>
