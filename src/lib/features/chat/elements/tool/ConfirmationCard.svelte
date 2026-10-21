<script lang="ts">
  import type { HTMLAttributes } from 'svelte/elements'
  import type { ChatPermissionOption } from '$lib/types/backend'
  import { Button } from '$lib/components/ui/button'
  import { cn } from '$lib/utils/cn'

  let {
    title,
    options,
    onRespond,
    class: className,
    ...rest
  }: Omit<HTMLAttributes<HTMLDivElement>, 'title'> & {
    title?: string
    options: ChatPermissionOption[]
    onRespond: (optionId: string) => void
  } = $props()
</script>

<div
  data-slot="tool-confirmation"
  class={cn(
    'mt-1 mb-1.5 ml-5 flex flex-col gap-2 rounded-lg border border-warning/40 bg-warning-bg px-3 py-2',
    className
  )}
  {...rest}
>
  {#if title}<p class="text-sm text-fg">{title}</p>{/if}
  <div class="flex flex-wrap gap-1.5">
    {#each options as option (option.option_id)}
      <Button
        size="xs"
        variant={option.kind === 'allow_once' ? 'accent' : option.kind.startsWith('reject_') ? 'outline' : 'default'}
        onclick={() => onRespond(option.option_id)}>{option.name}</Button
      >
    {/each}
  </div>
</div>
