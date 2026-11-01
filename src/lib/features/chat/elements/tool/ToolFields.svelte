<script lang="ts">
  import type { ToolStep } from '$lib/features/chat/turns'

  let { call }: { call: ToolStep } = $props()

  const input = $derived(call.arguments)
  const fields = $derived.by(() => {
    if (input === null || typeof input !== 'object' || Array.isArray(input)) return null
    return Object.entries(input as Record<string, unknown>)
  })
</script>

{#if fields?.length || (fields === null && input != null)}
  <div data-slot="tool-fields" class="min-w-0 font-mono text-xs text-fg">
    {#if fields}
      <dl class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1">
        {#each fields as [key, value] (key)}
          <dt class="wrap-anywhere text-muted">{key}</dt>
          <dd class="min-w-0">
            {#if value !== null && typeof value === 'object'}
              <pre class="wrap-anywhere whitespace-pre-wrap">{JSON.stringify(value, null, 2)}</pre>
            {:else}
              <span class="wrap-anywhere whitespace-pre-wrap">{value === '' ? '(empty)' : String(value)}</span>
            {/if}
          </dd>
        {/each}
      </dl>
    {:else}
      <pre class="wrap-anywhere whitespace-pre-wrap">{JSON.stringify(input, null, 2)}</pre>
    {/if}
  </div>
{/if}
