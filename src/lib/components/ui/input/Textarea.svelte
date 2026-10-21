<script lang="ts">
  import { cn } from '$lib/utils/cn'
  import { inputVariants } from './Input.svelte'
  import type { HTMLTextareaAttributes } from 'svelte/elements'

  let {
    class: className,
    value = $bindable(''),
    ref = $bindable<HTMLTextAreaElement | null>(null),
    ...rest
  }: Omit<HTMLTextareaAttributes, 'value'> & {
    class?: string
    value?: string
    ref?: HTMLTextAreaElement | null
  } = $props()
</script>

<textarea
  data-slot="textarea"
  bind:value
  bind:this={ref}
  class={cn(inputVariants(), 'resize-none', className)}
  {...rest}
></textarea>
