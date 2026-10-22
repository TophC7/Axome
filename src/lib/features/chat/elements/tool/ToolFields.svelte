<script lang="ts">
  import type { HTMLAttributes } from 'svelte/elements'
  import { cn } from '$lib/utils/cn'
  import { toolPreviewField } from './kinds'

  let {
    input,
    class: className,
    ...rest
  }: Omit<HTMLAttributes<HTMLDivElement>, 'children'> & {
    input: unknown
  } = $props()

  const fields = $derived.by(() => {
    if (input === null || typeof input !== 'object' || Array.isArray(input)) return null
    const previewKey = toolPreviewField(input)?.key
    return Object.entries(input as Record<string, unknown>).filter(([key]) => key !== previewKey && key !== 'i')
  })
</script>

{#if fields?.length || (fields === null && input != null)}
  <div data-slot="tool-fields" class={cn('min-w-0 font-mono text-xs text-fg', className)} {...rest}>
    {#if fields}
      <dl class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1">
        {#each fields as [key, value] (key)}
          <dt class="break-all text-muted">{key}</dt>
          <dd class="min-w-0">
            {#if value !== null && typeof value === 'object'}
              <pre class="max-h-96 overflow-auto break-words whitespace-pre-wrap">{JSON.stringify(value, null, 2)}</pre>
            {:else}
              <span class="break-words whitespace-pre-wrap">{value === '' ? '(empty)' : String(value)}</span>
            {/if}
          </dd>
        {/each}
      </dl>
    {:else}
      <pre class="max-h-96 overflow-auto break-words whitespace-pre-wrap">{JSON.stringify(input, null, 2)}</pre>
    {/if}
  </div>
{/if}
