<script lang="ts">
  import type { ComponentProps } from 'svelte'
  import * as Alert from '$lib/components/ui/alert'
  import type { TurnEndEntry } from '$lib/features/chat/turns'

  let {
    end,
    ...rest
  }: Omit<ComponentProps<typeof Alert.Root>, 'variant' | 'layout' | 'children'> & {
    end: TurnEndEntry | null
  } = $props()
  const text = $derived.by(() => {
    switch (end?.stop_reason) {
      case 'refusal':
        return 'The agent refused to continue.'
      case 'max_tokens':
        return 'Stopped at the output token limit.'
      case 'max_turn_requests':
        return 'Stopped at the turn request limit.'
      case 'error':
        return end.error || 'The turn failed.'
      default:
        return null
    }
  })
</script>

{#if text}
  <Alert.Root
    data-slot="turn-notice"
    variant={end?.stop_reason === 'error' ? 'danger' : 'warning'}
    layout="card"
    {...rest}
  >
    <Alert.Description>{text}</Alert.Description>
  </Alert.Root>
{/if}
