<!--
  @component
  Checkbox — bits-ui wrapper with Axome design tokens.

  Renders a 14×14 square with an accent fill when checked and a
  Hugeicons check glyph. Use inside a <label> or pair with a sibling
  span for the label text.
-->

<script lang="ts">
  import { Checkbox } from 'bits-ui'
  import { cn } from '$lib/utils/cn'
  import { CheckIcon, MinusIcon } from '@hugeicons/core-free-icons'
  import Icon from '$lib/icons/Icon.svelte'

  let {
    checked = $bindable(false),
    indeterminate = $bindable(false),
    disabled = false,
    class: className,
    onCheckedChange,
    ...rest
  }: {
    checked?: boolean
    indeterminate?: boolean
    disabled?: boolean
    class?: string
    onCheckedChange?: (v: boolean) => void
    [key: string]: unknown
  } = $props()
</script>

<Checkbox.Root
  data-slot="checkbox"
  bind:checked
  bind:indeterminate
  {disabled}
  {onCheckedChange}
  class={cn(
    'inline-flex h-[14px] w-[14px] shrink-0 items-center justify-center rounded-sm border border-edge bg-surface text-ground transition-colors',
    'hover:border-accent',
    'data-[state=checked]:border-accent data-[state=checked]:bg-accent data-[state=checked]:text-ground',
    'data-[state=indeterminate]:border-accent data-[state=indeterminate]:bg-accent data-[state=indeterminate]:text-ground',
    'focus-visible:shadow-focus-ring focus-visible:outline-none',
    'disabled:pointer-events-none disabled:opacity-50',
    className
  )}
  {...rest}
>
  {#snippet children({ checked: c, indeterminate: i })}
    {#if i}
      <Icon icon={MinusIcon} size={10} strokeWidth={3} />
    {:else if c}
      <Icon icon={CheckIcon} size={10} strokeWidth={3} />
    {/if}
  {/snippet}
</Checkbox.Root>
