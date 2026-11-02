<!--
  @component
  HarnessIcon — a harness's brand mark image, or its Hugeicons icon in the current text color.
-->

<script lang="ts">
  import type { HarnessIconSource } from '$lib/features/sessions/harnesses/catalog'
  import Icon from '$lib/icons/Icon.svelte'
  import { cn } from '$lib/utils/cn'

  let {
    icon,
    size,
    alt = '',
    class: className
  }: { icon: HarnessIconSource; size: number; alt?: string; class?: string } = $props()
</script>

{#if typeof icon === 'string'}
  <img src={icon} {alt} width={size} height={size} class={cn('shrink-0', className)} />
{:else}
  <Icon {icon} {size} class={cn('shrink-0', className)} aria-label={alt || undefined} />
{/if}
