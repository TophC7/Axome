<script lang="ts">
  import type { HTMLAttributes } from 'svelte/elements'
  import { IconButton } from '$lib/components/ui/button'
  import { CopyIcon } from '$lib/icons/lucideExports'
  import { formatTokens } from '$lib/features/chat/format'
  import type { MessageEntry, TurnEndEntry } from '$lib/features/chat/turns'
  import { copyToClipboard } from '$lib/utils/clipboard'
  import { cn } from '$lib/utils/cn'

  let {
    end,
    answer,
    class: className,
    ...rest
  }: HTMLAttributes<HTMLDivElement> & {
    end: TurnEndEntry | null
    answer: MessageEntry | null
  } = $props()
  const parts = $derived.by(() => {
    const parts: string[] = []
    if (end?.usage) {
      const usage = end.usage
      parts.push(`${formatTokens(usage.input)} in`, `${formatTokens(usage.output)} out`)
      if (usage.cache_read !== null) parts.push(`${formatTokens(usage.cache_read)} cache R`)
      if (usage.cache_write !== null) parts.push(`${formatTokens(usage.cache_write)} cache W`)
    }
    if (end?.stop_reason === 'cancelled') parts.push('Cancelled')
    return parts.join(' · ')
  })
</script>

{#if parts || answer}
  <div data-slot="turn-footer" class={cn('flex min-h-6 items-center justify-between gap-2', className)} {...rest}>
    <span class="font-mono text-xs text-subtle">{parts}</span>
    {#if answer}
      <div class="duration-fast opacity-0 transition-opacity group-hover/turn:opacity-100 focus-within:opacity-100">
        <IconButton
          tooltip="Copy Message"
          onclick={() => answer && copyToClipboard(answer.text, 'Copy message failed')}
        >
          <CopyIcon size={12} />
        </IconButton>
      </div>
    {/if}
  </div>
{/if}
