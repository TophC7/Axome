<!--
  @component
  SearchInput: input with a leading search icon; `children` renders trailing
  adornments (e.g. a filter menu trigger) positioned against the field.
-->

<script lang="ts">
  import type { Snippet } from 'svelte'
  import { SearchIcon } from '$lib/icons/lucideExports'
  import { cn } from '$lib/utils/cn'
  import Input from './Input.svelte'
  import type { HTMLInputAttributes } from 'svelte/elements'

  let {
    class: className,
    value = $bindable(''),
    ref = $bindable<HTMLInputElement | null>(null),
    children,
    ...rest
  }: Omit<HTMLInputAttributes, 'value' | 'children'> & {
    class?: string
    value?: string | number
    ref?: HTMLInputElement | null
    children?: Snippet
  } = $props()
</script>

<div data-slot="search-input" class="relative">
  <SearchIcon class="pointer-events-none absolute top-1/2 left-2 size-3 -translate-y-1/2 text-subtle" />
  <Input bind:value bind:ref class={cn('pl-7', className)} {...rest} />
  {#if children}{@render children()}{/if}
</div>
