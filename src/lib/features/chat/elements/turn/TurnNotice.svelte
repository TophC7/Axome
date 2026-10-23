<script lang="ts">
  import type { ComponentProps } from 'svelte'
  import * as Alert from '$lib/components/ui/alert'
  import type { Turn } from '$lib/features/chat/turns'

  let { turn, ...rest }: Omit<ComponentProps<typeof Alert.Root>, 'variant' | 'layout' | 'children'> & { turn: Turn } =
    $props()
  const notices = $derived.by(() => {
    const rows: { text: string; danger: boolean }[] = []
    for (const entry of turn.assistants) {
      const body = entry.body
      if (body.stopReason === 'error') rows.push({ text: body.error || 'The response failed.', danger: true })
      else if (body.stopReason === 'length') rows.push({ text: 'Stopped at the output token limit.', danger: false })
      else if (body.stopReason === 'aborted') rows.push({ text: 'The response was aborted.', danger: false })
    }
    const end = turn.end
    if (end && end.outcome !== 'done') {
      const fallback = {
        aborted: 'The turn was aborted.',
        failed: 'The turn failed.',
        interrupted: 'The turn was interrupted.'
      }
      const text = end.reason || fallback[end.outcome]
      if (!rows.some((row) => row.text === text)) rows.push({ text, danger: end.outcome === 'failed' })
    }
    return rows
  })
</script>

{#each notices as notice}
  <Alert.Root data-slot="turn-notice" variant={notice.danger ? 'danger' : 'warning'} layout="card" {...rest}>
    <Alert.Description>{notice.text}</Alert.Description>
  </Alert.Root>
{/each}
